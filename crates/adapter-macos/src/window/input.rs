use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::bridge::run_ax;
use super::input_lock::input_lock_path;
use super::input_target::InputTarget;
use super::target::{WindowIdentity, now_seconds};
use crate::MacosError;
use crate::screenshot::{WindowScope, list_windows};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InputDelivery {
    SentUnverified,
    PartiallySent,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackgroundInputResult {
    pub action: String,
    pub window_id: u32,
    pub status: InputDelivery,
    pub events_sent: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interruption: Option<String>,
    pub frontmost_pid_before: i32,
    pub frontmost_pid_after: i32,
    pub foreground_changed: bool,
}

fn send(target: InputTarget, mut request: Value) -> Result<BackgroundInputResult, MacosError> {
    let current = list_windows(WindowScope::AllSpaces)?
        .into_iter()
        .find(|window| window.window_id == target.window.window_id)
        .ok_or_else(|| MacosError::NotFound("window disappeared; take a new snapshot".into()))?;
    if WindowIdentity::from(&current) != target.window {
        return Err(MacosError::Other(
            "window changed; take a new snapshot".into(),
        ));
    }
    request["issued_at"] = json!(target.issued_at);
    request["caller_pid"] = json!(std::process::id());
    let lock_path = input_lock_path(target.window.owner_pid)?;
    request["lock_path"] = json!(
        lock_path
            .to_str()
            .ok_or_else(|| MacosError::Other("input lock path must be valid UTF-8".into()))?
    );
    run_ax(
        &target.window,
        include_str!("background_input.swift"),
        &[],
        &request,
    )
}

/// Send Unicode keyboard events to the background app's verified focused window.
pub fn type_text(token: &str, text: &str) -> Result<BackgroundInputResult, MacosError> {
    if text.is_empty() || text.encode_utf16().count() > 1024 || text.chars().any(char::is_control) {
        return Err(MacosError::Other("text must contain 1..=1024 UTF-16 units without control characters; use key for Tab or Enter".into()));
    }
    let target = InputTarget::decode(token, now_seconds()?)?;
    send(target, json!({"action": "type_text", "text": text}))
}

fn key_code(name: &str) -> Option<u16> {
    Some(match name {
        "tab" => 48,
        "enter" => 36,
        "escape" => 53,
        "backspace" => 51,
        "delete" => 117,
        "left" => 123,
        "right" => 124,
        "down" => 125,
        "up" => 126,
        "home" => 115,
        "end" => 119,
        "page-up" => 116,
        "page-down" => 121,
        "space" => 49,
        "a" => 0,
        "b" => 11,
        "c" => 8,
        "d" => 2,
        "e" => 14,
        "f" => 3,
        "g" => 5,
        "h" => 4,
        "i" => 34,
        "j" => 38,
        "k" => 40,
        "l" => 37,
        "m" => 46,
        "n" => 45,
        "o" => 31,
        "p" => 35,
        "q" => 12,
        "r" => 15,
        "s" => 1,
        "t" => 17,
        "u" => 32,
        "v" => 9,
        "w" => 13,
        "x" => 7,
        "y" => 16,
        "z" => 6,
        "0" => 29,
        "1" => 18,
        "2" => 19,
        "3" => 20,
        "4" => 21,
        "5" => 23,
        "6" => 22,
        "7" => 26,
        "8" => 28,
        "9" => 25,
        _ => return None,
    })
}

fn modifier_flags(modifiers: &[String]) -> Result<u64, MacosError> {
    let mut flags = 0;
    for modifier in modifiers {
        flags |= match modifier.as_str() {
            "shift" => 1 << 17,
            "control" => 1 << 18,
            "option" => 1 << 19,
            "command" => 1 << 20,
            _ => {
                return Err(MacosError::Other(format!(
                    "unsupported key modifier: {modifier}"
                )));
            }
        };
    }
    Ok(flags)
}

/// Send a named key pair and optional modifiers to the verified background keyboard window.
pub fn key(
    token: &str,
    name: &str,
    modifiers: &[String],
) -> Result<BackgroundInputResult, MacosError> {
    let code =
        key_code(name).ok_or_else(|| MacosError::Other(format!("unsupported key: {name}")))?;
    let flags = modifier_flags(modifiers)?;
    let target = InputTarget::decode(token, now_seconds()?)?;
    send(
        target,
        json!({"action": "key", "key_code": code, "flags": flags}),
    )
}

/// Send pixel-unit wheel deltas at a point in the snapshot; positive deltas scroll up/left.
pub fn scroll(
    token: &str,
    x: f64,
    y: f64,
    delta_x: i32,
    delta_y: i32,
) -> Result<BackgroundInputResult, MacosError> {
    if delta_x.unsigned_abs() > 4096
        || delta_y.unsigned_abs() > 4096
        || (delta_x == 0 && delta_y == 0)
    {
        return Err(MacosError::Other(
            "scroll deltas must be within -4096..=4096 and not both zero".into(),
        ));
    }
    let target = InputTarget::decode(token, now_seconds()?)?;
    let (frame_x, frame_y) = target.frame_point(x, y)?;
    send(
        target,
        json!({"action": "scroll", "x": frame_x, "y": frame_y, "delta_x": delta_x, "delta_y": delta_y}),
    )
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
