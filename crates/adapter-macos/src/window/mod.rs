use serde::{Deserialize, Serialize};

use crate::MacosError;
use crate::screenshot::{
    CapturableWindow, ScreenshotResult, capture_window, list_capturable_windows,
};

mod actions;
mod bridge;
mod target;

pub use actions::{ActionStatus, WindowActionResult, press, set_value};
use bridge::run_ax;
use target::{Target, WindowIdentity, now_seconds};

#[cfg(test)]
mod tests;

#[derive(Debug, Serialize, Deserialize)]
pub struct AccessibilityNode {
    pub r#ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_ref: Option<String>,
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subrole: Option<String>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    pub actions: Vec<String>,
    pub settable_value: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing)]
    fingerprint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AccessibilitySnapshot {
    pub window_id: u32,
    pub owner_pid: i32,
    pub nodes: Vec<AccessibilityNode>,
    pub truncated: bool,
}

#[derive(Debug, Serialize)]
pub struct WindowInspectResult {
    pub window: CapturableWindow,
    pub accessibility: AccessibilitySnapshot,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<ScreenshotResult>,
}

fn run_ax_inspect(
    window: &CapturableWindow,
    limit: usize,
    max_depth: usize,
) -> Result<AccessibilitySnapshot, MacosError> {
    let identity = WindowIdentity::from(window);
    let issued_at = now_seconds()?;
    let mut snapshot: AccessibilitySnapshot = run_ax(
        &identity,
        include_str!("ax_inspect.swift"),
        &[limit.to_string(), max_depth.to_string()],
        &serde_json::Value::Null,
    )?;
    if snapshot.window_id != window.window_id || snapshot.owner_pid != window.owner_pid {
        return Err(MacosError::Other(
            "window identity changed during accessibility inspection".into(),
        ));
    }
    for node in &mut snapshot.nodes {
        let can_set_text =
            node.settable_value && matches!(node.role.as_str(), "AXTextField" | "AXTextArea");
        if node.enabled != Some(false)
            && (can_set_text || node.actions.iter().any(|action| action == "AXPress"))
        {
            node.target = Some(
                Target {
                    version: 1,
                    issued_at,
                    window: identity.clone(),
                    r#ref: node.r#ref.clone(),
                    fingerprint: node.fingerprint.clone(),
                }
                .encode()?,
            );
        }
    }
    Ok(snapshot)
}

/// Inspect one exact on-screen window through macOS Accessibility, with optional screenshot and OCR.
pub fn inspect_window(
    window_id: u32,
    limit: usize,
    max_depth: usize,
    screenshot: bool,
    ocr: bool,
) -> Result<WindowInspectResult, MacosError> {
    if !(1..=500).contains(&limit) {
        return Err(MacosError::Other(
            "window inspect limit must be 1..=500".into(),
        ));
    }
    if !(1..=12).contains(&max_depth) {
        return Err(MacosError::Other(
            "window inspect depth must be 1..=12".into(),
        ));
    }

    let window = list_capturable_windows()?
        .into_iter()
        .find(|window| window.window_id == window_id)
        .ok_or_else(|| {
            MacosError::NotFound(format!("capturable window id not found: {window_id}"))
        })?;
    let accessibility = run_ax_inspect(&window, limit, max_depth)?;
    let screenshot = if screenshot || ocr {
        Some(capture_window(ocr, None, window_id)?)
    } else {
        None
    };
    Ok(WindowInspectResult {
        window,
        accessibility,
        screenshot,
    })
}
