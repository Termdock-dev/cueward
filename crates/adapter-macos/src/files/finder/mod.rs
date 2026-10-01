//! Scoped Finder observations and explicit desktop-changing reveal requests.
mod model;
mod paths;
#[cfg(test)]
mod script_tests;
#[cfg(test)]
mod protocol_tests;
#[cfg(test)]
mod tests;
mod workspace;

use super::{MacFiles, policy, protocol};
use cueward_core::files::*;
pub use model::*;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;

trait Native {
    fn finder_pid(&self) -> Result<Option<i32>, FileError>;
    fn foreground_pid(&self) -> Option<i32>;
    fn context(&self, max_items: usize) -> Result<RawContext, FileError>;
    fn reveal(&self, path: &Path) -> Result<(), FileError>;
}

struct System;
impl Native for System {
    fn finder_pid(&self) -> Result<Option<i32>, FileError> {
        workspace::finder_pid()
    }
    fn foreground_pid(&self) -> Option<i32> {
        workspace::foreground_pid()
    }
    fn context(&self, max_items: usize) -> Result<RawContext, FileError> {
        // Inherit the worker's process group so the parent deadline stops both.
        let output = Command::new("/usr/bin/osascript")
            .args(["-l", "JavaScript", "-e", include_str!("context.js"), "--"])
            .arg(max_items.to_string())
            .output()?;
        if !output.status.success() {
            return Err(FileError::new(
                FileErrorCode::Unavailable,
                format!(
                    "Finder script failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            ));
        }
        if output.stdout.len() > 16 * 1024 * 1024 {
            return Err(FileError::new(
                FileErrorCode::ScanLimit,
                "Finder response exceeds 16 MiB",
            ));
        }
        serde_json::from_slice::<Result<RawContext, FileError>>(&output.stdout).map_err(|e| {
            FileError::new(
                FileErrorCode::Internal,
                format!("invalid Finder response: {e}"),
            )
        })?
    }
    fn reveal(&self, path: &Path) -> Result<(), FileError> {
        workspace::reveal(path)
    }
}

/// Run a Finder worker with the same request/response bounds and deadline as files.
pub fn run(
    executable: &Path,
    request: &FinderRequest,
    timeout_ms: u64,
) -> Result<FinderResponse, FileError> {
    let mut result = protocol::run_typed(executable, "files-finder-worker", request, timeout_ms);
    if matches!(request.action, FinderAction::Reveal { .. }) {
        if let Err(error) = &mut result {
            if matches!(
                error.code,
                FileErrorCode::Timeout | FileErrorCode::Internal | FileErrorCode::Io
            ) {
                error
                    .message
                    .push_str("; reveal may have been delivered; inspect Finder before retrying");
            }
        }
    }
    result
}

/// Execute only inside the deadline-controlled worker; no-materialization applies
/// to this process's file observations, not to Finder's own preview behavior.
pub fn execute_worker(request: &FinderRequest) -> Result<FinderResponse, FileError> {
    let _policy = policy::NoMaterialization::enter()?;
    execute(request, &System, None)
}

fn execute(
    request: &FinderRequest,
    native: &impl Native,
    lock_dir: Option<&Path>,
) -> Result<FinderResponse, FileError> {
    let root = observe(&request.root, Path::new("."), false, None)?;
    match &request.action {
        FinderAction::Context { max_items } => context(request, root, *max_items, native),
        FinderAction::Reveal { .. } => reveal(request, root, native, lock_dir),
    }
}

fn observe(
    root: &Path,
    path: &Path,
    follow_links: bool,
    expected_version: Option<String>,
) -> Result<FileInfo, FileError> {
    let request = FileRequest {
        root: root.to_owned(),
        path: path.to_owned(),
        follow_links,
        expected_version,
        action: FileAction::Info,
    };
    let FileResponse::Info(file) = cueward_core::files::execute(&MacFiles, &request)? else {
        return Err(FileError::new(
            FileErrorCode::Internal,
            "expected file observation",
        ));
    };
    Ok(file)
}

fn check_root(request: &FinderRequest, root: &FileInfo) -> Result<(), FileError> {
    let after = observe(
        &request.root,
        Path::new("."),
        false,
        Some(root.version.clone()),
    )?;
    if after.path != root.path {
        return Err(FileError::new(
            FileErrorCode::Changed,
            "root path changed during Finder operation",
        ));
    }
    Ok(())
}

fn foreground(before: Option<i32>, native: &impl Native) -> ForegroundObservation {
    let after = native.foreground_pid();
    ForegroundObservation {
        frontmost_pid_before: before,
        frontmost_pid_after: after,
        foreground_changed: before.zip(after).map(|(a, b)| a != b),
    }
}

fn context(
    request: &FinderRequest,
    root: FileInfo,
    max_items: usize,
    native: &impl Native,
) -> Result<FinderResponse, FileError> {
    if !(1..=500).contains(&max_items) {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "max-items must be 1..500",
        ));
    }
    let before = native.foreground_pid();
    let (pid, raw) = read_context(max_items, native)?;
    let count = raw.as_ref().map(|r| r.window_count);
    let (window, selection) = raw
        .map(|r| {
            let window = r.front_window.map(|w| FinderWindow {
                id: w.id,
                location: paths::item(request, &root, w.location),
            });
            let selection = r
                .selection
                .into_iter()
                .map(|i| paths::item(request, &root, i))
                .collect();
            (window, Some(selection))
        })
        .unwrap_or((None, None));
    check_root(request, &root)?;
    Ok(FinderResponse::FinderContext(FinderContext {
        root,
        finder_pid: pid,
        window_count: count,
        front_window: window,
        selection,
        selection_complete: pid.is_some(),
        activation_requested: false,
        foreground: foreground(before, native),
    }))
}

fn read_context(
    max_items: usize,
    native: &impl Native,
) -> Result<(Option<i32>, Option<RawContext>), FileError> {
    let pid = native.finder_pid()?;
    let raw = if pid.is_some() {
        Some(native.context(max_items)?)
    } else {
        None
    };
    if native.finder_pid()? != pid {
        return Err(FileError::new(
            FileErrorCode::Changed,
            "Finder process changed during observation",
        ));
    }
    if raw.as_ref().is_some_and(|r| r.selection.len() > max_items) {
        return Err(FileError::new(
            FileErrorCode::ScanLimit,
            "Finder selection exceeds max-items",
        ));
    }
    Ok((pid, raw))
}

struct RevealLocks {
    _global: File,
    _recipient: Option<File>,
}

fn locks(pid: Option<i32>, directory: Option<&Path>) -> Result<RevealLocks, FileError> {
    let supplied = directory.is_some();
    let directory = match directory {
        Some(path) => path.to_owned(),
        None => PathBuf::from(crate::screenshot::ensure_cache_dir().map_err(lock_error)?),
    };
    let global =
        crate::window::lock_path(&directory.join("finder-reveal.lock")).map_err(lock_error)?;
    let recipient = pid
        .map(|pid| {
            if supplied {
                crate::window::lock_path(&directory.join(format!("input-{pid}.lock")))
            } else {
                crate::window::lock_input(pid)
            }
        })
        .transpose()
        .map_err(lock_error)?;
    Ok(RevealLocks {
        _global: global,
        _recipient: recipient,
    })
}

fn lock_error(error: impl std::fmt::Display) -> FileError {
    FileError::new(
        FileErrorCode::Unavailable,
        format!("reveal was not submitted: {error}"),
    )
}

fn reveal(
    request: &FinderRequest,
    root: FileInfo,
    native: &impl Native,
    directory: Option<&Path>,
) -> Result<FinderResponse, FileError> {
    let (file, path, follow) = prepare_reveal(request)?;
    let pid = native.finder_pid()?;
    let _locks = locks(pid, directory)?;
    if native.finder_pid()? != pid {
        return Err(FileError::new(
            FileErrorCode::Changed,
            "Finder recipient changed before reveal; request was not submitted",
        ));
    }
    check_root(request, &root)?;
    observe(&request.root, path, follow, Some(file.version.clone()))?;
    let before = native.foreground_pid();
    native.reveal(Path::new(&file.path))?;
    let post_check = check_root(request, &root)
        .and_then(|()| observe(&request.root, path, follow, Some(file.version.clone())));
    Ok(FinderResponse::FinderReveal(FinderReveal {
        status: RevealStatus::SentUnverified,
        file,
        activation_requested: true,
        selection_change_requested: true,
        foreground: foreground(before, native),
        post_check,
    }))
}

fn prepare_reveal(request: &FinderRequest) -> Result<(FileInfo, &Path, bool), FileError> {
    let FinderAction::Reveal {
        path,
        follow_links,
        expected_version,
    } = &request.action
    else {
        return Err(FileError::new(
            FileErrorCode::Internal,
            "expected reveal request",
        ));
    };
    let follow = *follow_links;
    let file = observe(&request.root, path, follow, expected_version.clone())?;
    require_revealable(&file)?;
    Ok((file, path, follow))
}

fn require_revealable(file: &FileInfo) -> Result<(), FileError> {
    if !matches!(file.kind, FileKind::File | FileKind::Directory) {
        return Err(FileError::new(
            FileErrorCode::UnsupportedType,
            "reveal requires a regular file/directory; symlinks require scoped --follow-links",
        ));
    }
    if file.data_state == DataState::Dataless {
        return Err(FileError::new(
            FileErrorCode::Unavailable,
            "dataless item was not revealed; no download is requested",
        ));
    }
    Ok(())
}
