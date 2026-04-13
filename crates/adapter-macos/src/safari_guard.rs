use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::MacosError;

pub(crate) const SAFARI_LOCK_TTL_SECS: i64 = 1800;

static SAFARI_AUTOMATION_STATE: OnceLock<Mutex<SafariAutomationState>> = OnceLock::new();

#[derive(Debug, Default)]
pub(crate) struct SafariAutomationState {
    pub(crate) depth: usize,
    pub(crate) last_operation_at: Option<std::time::Instant>,
    pub(crate) lock_path: Option<PathBuf>,
    pub(crate) lock_owner_pid: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SafariLockFile {
    pub(crate) pid: u32,
    pub(crate) acquired_at: i64,
    pub(crate) expires_at: i64,
}

pub(crate) struct SafariAutomationSession {
    pub(crate) outermost: bool,
}

impl SafariAutomationSession {
    pub(crate) fn enter() -> Result<Self, MacosError> {
        let state = safari_automation_state();
        let mut guard = state
            .lock()
            .map_err(|_| MacosError::Other("safari automation state poisoned".to_string()))?;

        if guard.depth > 0 {
            guard.depth += 1;
            return Ok(Self { outermost: false });
        }

        let lock_path = safari_lock_path()?;
        acquire_safari_lock(&lock_path, Utc::now().timestamp(), std::process::id())?;
        guard.depth = 1;
        guard.lock_owner_pid = Some(std::process::id());
        guard.lock_path = Some(lock_path);

        Ok(Self { outermost: true })
    }
}

impl Drop for SafariAutomationSession {
    fn drop(&mut self) {
        let state = safari_automation_state();
        let Ok(mut guard) = state.lock() else {
            return;
        };

        if guard.depth == 0 {
            return;
        }

        guard.depth -= 1;
        if !self.outermost || guard.depth > 0 {
            return;
        }

        if let (Some(path), Some(pid)) = (guard.lock_path.as_ref(), guard.lock_owner_pid) {
            let _ = release_safari_lock(path, pid);
        }
        guard.lock_path = None;
        guard.lock_owner_pid = None;
    }
}

pub(crate) fn safari_automation_state() -> &'static Mutex<SafariAutomationState> {
    SAFARI_AUTOMATION_STATE.get_or_init(|| Mutex::new(SafariAutomationState::default()))
}

pub(crate) fn with_safari_session<T>(
    action: impl FnOnce() -> Result<T, MacosError>,
) -> Result<T, MacosError> {
    let _session = SafariAutomationSession::enter()?;
    action()
}

fn safari_lock_path() -> Result<PathBuf, MacosError> {
    let home = std::env::var("HOME")
        .map_err(|_| MacosError::Other("HOME environment variable must be set".into()))?;
    Ok(PathBuf::from(home).join(".cueward").join("lock.json"))
}

pub(crate) fn acquire_safari_lock(path: &Path, now_ts: i64, pid: u32) -> Result<(), MacosError> {
    let parent = path
        .parent()
        .ok_or_else(|| MacosError::Other("invalid Safari lock path".to_string()))?;
    fs::create_dir_all(parent)
        .map_err(|e| MacosError::Other(format!("failed to create {}: {e}", parent.display())))?;

    for _ in 0..2 {
        match OpenOptions::new().write(true).create_new(true).open(path) {
            Ok(mut file) => {
                let payload = SafariLockFile {
                    pid,
                    acquired_at: now_ts,
                    expires_at: now_ts + SAFARI_LOCK_TTL_SECS,
                };
                let bytes = serde_json::to_vec_pretty(&payload)
                    .map_err(|e| MacosError::Other(format!("failed to encode lock file: {e}")))?;
                file.write_all(&bytes).map_err(|e| {
                    let _ = fs::remove_file(path);
                    MacosError::Other(format!("failed to write {}: {e}", path.display()))
                })?;
                return Ok(());
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                let existing_raw = match fs::read_to_string(path) {
                    Ok(raw) => raw,
                    Err(read_error) if read_error.kind() == ErrorKind::NotFound => continue,
                    Err(read_error) => {
                        return Err(MacosError::Other(format!(
                            "failed to read Safari lock {}: {read_error}",
                            path.display()
                        )));
                    }
                };
                let existing: SafariLockFile =
                    serde_json::from_str(&existing_raw).map_err(|_| {
                        MacosError::Other(format!(
                            "Safari automation lock {} exists but is corrupted or unreadable",
                            path.display()
                        ))
                    })?;
                let expired = existing.expires_at <= now_ts;
                if expired {
                    match fs::remove_file(path) {
                        Ok(()) => continue,
                        Err(remove_error) if remove_error.kind() == ErrorKind::NotFound => continue,
                        Err(remove_error) => {
                            return Err(MacosError::Other(format!(
                                "failed to clear stale Safari lock {}: {remove_error}",
                                path.display()
                            )));
                        }
                    }
                }

                return Err(MacosError::Other(format!(
                    "Safari automation is locked by pid {} until {}",
                    existing.pid, existing.expires_at
                )));
            }
            Err(error) => {
                return Err(MacosError::Other(format!(
                    "failed to create Safari lock {}: {error}",
                    path.display()
                )));
            }
        }
    }

    Err(MacosError::Other(format!(
        "failed to acquire Safari lock {}",
        path.display()
    )))
}

pub(crate) fn release_safari_lock(path: &Path, pid: u32) -> Result<(), MacosError> {
    let owner_matches = read_safari_lock(path)
        .map(|lock| lock.pid == pid)
        .unwrap_or(false);
    if !owner_matches {
        return Ok(());
    }
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(MacosError::Other(format!(
            "failed to remove Safari lock {}: {err}",
            path.display()
        ))),
    }
}

pub(crate) fn read_safari_lock(path: &Path) -> Option<SafariLockFile> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}
