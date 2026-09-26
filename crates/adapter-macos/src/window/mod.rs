use std::io::Write;
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::MacosError;
use crate::screenshot::{
    CapturableWindow, ScreenshotResult, capture_window, list_capturable_windows,
};

const AX_INSPECT_SCRIPT: &str = include_str!("ax_inspect.swift");

#[cfg(test)]
mod tests;

#[derive(Debug, Serialize, Deserialize)]
pub struct AccessibilityNode {
    pub r#ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_ref: Option<String>,
    pub role: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    pub actions: Vec<String>,
    pub settable_value: bool,
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
    let mut script = tempfile::NamedTempFile::with_suffix(".swift")
        .map_err(|error| MacosError::Other(format!("failed to create AX script: {error}")))?;
    script
        .write_all(AX_INSPECT_SCRIPT.as_bytes())
        .map_err(|error| MacosError::Other(format!("failed to write AX script: {error}")))?;
    let output = Command::new("swift")
        .arg(script.path())
        .arg(window.owner_pid.to_string())
        .arg(window.window_id.to_string())
        .arg(&window.title)
        .arg(window.bounds.x.to_string())
        .arg(window.bounds.y.to_string())
        .arg(window.bounds.width.to_string())
        .arg(window.bounds.height.to_string())
        .arg(limit.to_string())
        .arg(max_depth.to_string())
        .output()
        .map_err(|error| MacosError::Other(format!("swift AX inspection failed: {error}")))?;

    if !output.status.success() {
        if output.status.code() == Some(3) {
            return Err(MacosError::Other(
                "Accessibility permission is required for window inspection; allow the terminal app in System Settings > Privacy & Security > Accessibility".into(),
            ));
        }
        let message = String::from_utf8_lossy(&output.stderr);
        return Err(MacosError::Other(format!(
            "window accessibility inspection failed: {}",
            message.trim()
        )));
    }

    let snapshot: AccessibilitySnapshot = serde_json::from_slice(&output.stdout)
        .map_err(|error| MacosError::Other(format!("invalid AX snapshot: {error}")))?;
    if snapshot.window_id != window.window_id || snapshot.owner_pid != window.owner_pid {
        return Err(MacosError::Other(
            "window identity changed during accessibility inspection".into(),
        ));
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
