use serde::{Deserialize, Serialize};

use super::bridge::run_ax;
use super::target::{Target, WindowIdentity, now_seconds};
use crate::MacosError;
use crate::screenshot::list_capturable_windows;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionStatus {
    Confirmed,
    SentUnverified,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WindowActionResult {
    pub action: String,
    pub window_id: u32,
    pub r#ref: String,
    pub status: ActionStatus,
    pub frontmost_pid_before: i32,
    pub frontmost_pid_after: i32,
    pub foreground_changed: bool,
}

fn act(token: &str, action: &str, value: Option<&str>) -> Result<WindowActionResult, MacosError> {
    let target = Target::decode(token, now_seconds()?)?;
    let windows = list_capturable_windows()?;
    let current = windows
        .iter()
        .find(|window| window.window_id == target.window.window_id)
        .ok_or_else(|| MacosError::NotFound("window disappeared; inspect again".into()))?;
    if WindowIdentity::from(current) != target.window {
        return Err(MacosError::Other(
            "window identity changed; inspect again".into(),
        ));
    }
    run_ax(
        &target.window,
        include_str!("ax_action.swift"),
        &[],
        &serde_json::json!({
            "ref": target.r#ref,
            "fingerprint": target.fingerprint,
            "issued_at": target.issued_at,
            "action": action,
            "value": value,
        }),
    )
}

/// Press an inspected AX element without activating the app or injecting global input.
pub fn press(token: &str) -> Result<WindowActionResult, MacosError> {
    act(token, "press", None)
}

/// Set an inspected text field's AXValue and read back whether it matches.
pub fn set_value(token: &str, value: &str) -> Result<WindowActionResult, MacosError> {
    if value.len() > 65_536 {
        return Err(MacosError::Other(
            "window text value must be at most 65536 bytes".into(),
        ));
    }
    act(token, "set_value", Some(value))
}
