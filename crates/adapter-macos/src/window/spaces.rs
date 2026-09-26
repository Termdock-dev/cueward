use std::fs;
use std::process::Command;
use std::time::Duration;

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

use super::input_lock::input_lock_path;
use super::input_target::InputTarget;
use super::process::run_with_timeout;
use super::target::{WindowIdentity, now_seconds};
use super::{WindowScope, list_windows};
use crate::MacosError;

#[derive(Debug, Serialize, Deserialize)]
pub struct SpaceInfo {
    pub id: u64,
    pub r#type: i32,
    pub is_visible: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SpaceDisplay {
    pub id: String,
    pub current_space: u64,
    pub spaces: Vec<SpaceInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SpaceCatalog {
    pub displays: Vec<SpaceDisplay>,
    pub move_window_available: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WindowSpaces {
    pub window_id: u32,
    pub space_ids: Vec<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SpaceMoveResult {
    pub window_id: u32,
    pub space_id: u64,
    pub status: super::ActionStatus,
    pub before_spaces: Vec<u64>,
    pub after_spaces: Vec<u64>,
    pub window_changed: bool,
    pub frontmost_pid_before: i32,
    pub frontmost_pid_after: i32,
    pub foreground_changed: bool,
    pub visible_spaces_before: Vec<u64>,
    pub visible_spaces_after: Vec<u64>,
    pub visible_spaces_changed: bool,
}

fn run<T: DeserializeOwned>(request: Value) -> Result<T, MacosError> {
    let source = format!(
        "{}\n{}",
        include_str!("space_guards.m"),
        include_str!("spaces.m")
    );
    let output = run_objc(
        &source,
        &serde_json::to_vec(&request)
            .map_err(|e| MacosError::Other(format!("invalid Space request: {e}")))?,
    )?;
    serde_json::from_slice(&output)
        .map_err(|e| MacosError::Other(format!("invalid Space result: {e}")))
}

fn run_objc(source: &str, payload: &[u8]) -> Result<Vec<u8>, MacosError> {
    let directory = tempfile::tempdir().map_err(|e| MacosError::Other(e.to_string()))?;
    let path = directory.path().join("spaces.m");
    let binary = directory.path().join("spaces-helper");
    fs::write(&path, source).map_err(|e| MacosError::Other(e.to_string()))?;
    let compiled = run_with_timeout(
        Command::new("clang")
            .args(["-fobjc-arc", "-framework", "Cocoa"])
            .arg(&path)
            .arg("-o")
            .arg(&binary),
        b"",
        Duration::from_secs(40),
    )
    .map_err(|e| {
        MacosError::Other(format!(
            "Space helper requires the macOS command line tools: {e}"
        ))
    })?;
    if !compiled.status.success() {
        return Err(MacosError::Other(format!(
            "Space helper compilation failed: {}",
            String::from_utf8_lossy(&compiled.stderr)
        )));
    }
    let output = run_with_timeout(&mut Command::new(binary), payload, Duration::from_secs(25))
        .map_err(|e| {
            MacosError::Other(format!("Space helper failed: {e}; observe before retrying"))
        })?;
    if !output.status.success() {
        return Err(MacosError::Other(format!(
            "Space operation failed: {}; observe before retrying",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(output.stdout)
}

/// Query existing macOS managed Spaces without switching the visible desktop.
pub fn list_spaces() -> Result<SpaceCatalog, MacosError> {
    run(json!({"action": "list"}))
}

/// Read membership for one current, titled application window.
pub fn window_spaces(window_id: u32) -> Result<WindowSpaces, MacosError> {
    if window_id == 0 {
        return Err(MacosError::Other("window id must be positive".into()));
    }
    let window = list_windows(WindowScope::AllSpaces)?
        .into_iter()
        .find(|w| w.window_id == window_id)
        .ok_or_else(|| MacosError::NotFound("window disappeared; observe again".into()))?;
    run(json!({"action": "membership", "window": WindowIdentity::from(&window)}))
}

/// Move a snapshot-bound background window to an existing inactive user Space and read back membership.
pub fn move_window_to_space(token: &str, space_id: u64) -> Result<SpaceMoveResult, MacosError> {
    if space_id == 0 {
        return Err(MacosError::Other("Space id must be positive".into()));
    }
    let target = InputTarget::decode(token, now_seconds()?)?;
    let lock = input_lock_path(target.window.owner_pid)?;
    let lock = lock
        .to_str()
        .ok_or_else(|| MacosError::Other("input lock path must be UTF-8".into()))?;
    run(json!({
        "action": "move", "window": target.window, "issued_at": target.issued_at,
        "space_id": space_id, "caller_pid": std::process::id(), "lock_path": lock,
    }))
}

#[cfg(test)]
#[path = "spaces_tests.rs"]
mod tests;
