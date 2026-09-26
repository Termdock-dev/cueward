//! Generic application discovery, background launch, and Accessibility exploration.
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

use crate::MacosError;
use crate::window::process::run_with_timeout;

mod accessibility;
pub use accessibility::{
    AppAXActionResult, AppAccessibilityNode, AppAccessibilitySnapshot, AppInstance, inspect_app,
    press_app_element, set_app_value,
};

#[derive(Debug, Deserialize, Serialize)]
pub struct RunningApp {
    pub pid: i32,
    pub name: String,
    pub bundle_id: Option<String>,
    pub path: Option<String>,
    pub is_active: bool,
    pub is_hidden: bool,
    pub finished_launching: bool,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LaunchStatus {
    Launched,
    AlreadyRunning,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LaunchResult {
    pub status: LaunchStatus,
    pub app: RunningApp,
    pub frontmost_pid_before: i32,
    pub frontmost_pid_after: i32,
    pub foreground_changed: bool,
}

fn run<T: DeserializeOwned>(request: Value) -> Result<T, MacosError> {
    run_source(request, &helper_source())
}

fn run_source<T: DeserializeOwned>(request: Value, source: &str) -> Result<T, MacosError> {
    let directory = tempfile::tempdir().map_err(|e| MacosError::Other(e.to_string()))?;
    let script = directory.path().join("apps.swift");
    fs::write(&script, source).map_err(|e| MacosError::Other(e.to_string()))?;
    let payload = serde_json::to_vec(&request).map_err(|e| MacosError::Other(e.to_string()))?;
    let output = run_with_timeout(
        Command::new("swift").arg(script),
        &payload,
        Duration::from_secs(40),
    )
    .map_err(|e| MacosError::Other(format!("app helper failed: {e}; list apps before retrying")))?;
    if !output.status.success() {
        return Err(MacosError::Other(format!(
            "app operation failed: {}; list apps before retrying",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    serde_json::from_slice(&output.stdout).map_err(|e| {
        MacosError::Other(format!(
            "invalid app result: {e}; list apps before retrying"
        ))
    })
}

fn helper_source() -> String {
    format!(
        "{}\n{}",
        include_str!("completion.swift"),
        include_str!("apps.swift")
    )
}

/// List running regular and accessory applications without activating them.
pub fn list_apps() -> Result<Vec<RunningApp>, MacosError> {
    run(json!({"action": "list"}))
}

fn launch_request(bundle_id: Option<&str>, path: Option<&Path>) -> Result<Value, MacosError> {
    let mut request = json!({"action": "launch", "caller_pid": std::process::id()});
    match (bundle_id, path) {
        (Some(id), None)
            if !id.is_empty()
                && id.len() <= 255
                && id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b".-".contains(&c)) =>
        {
            request["bundle_id"] = json!(id)
        }
        (None, Some(path))
            if path.is_absolute() && path.extension().is_some_and(|e| e == "app") =>
        {
            let path = path
                .to_str()
                .ok_or_else(|| MacosError::Other("app path must be UTF-8".into()))?;
            request["path"] = json!(path);
        }
        _ => {
            return Err(MacosError::Other(
                "provide one valid bundle id or absolute .app path".into(),
            ));
        }
    }
    Ok(request)
}

/// Request a background launch, returning existing instances without sending a reopen event.
pub fn launch_app(
    bundle_id: Option<&str>,
    path: Option<&Path>,
) -> Result<LaunchResult, MacosError> {
    run(launch_request(bundle_id, path)?)
}

#[cfg(test)]
mod tests;
