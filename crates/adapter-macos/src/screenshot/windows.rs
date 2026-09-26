use serde::{Deserialize, Serialize};
use std::io::Write;
use std::process::Command;

use crate::MacosError;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CapturableWindow {
    pub window_id: u32,
    pub app: String,
    pub title: String,
    pub owner_pid: i32,
    pub is_frontmost: bool,
    pub is_onscreen: bool,
    pub bounds: WindowBounds,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub(crate) struct WindowCatalogEntry {
    pub window_id: u32,
    pub app: String,
    pub title: String,
    pub owner_pid: i32,
    pub layer: i32,
    pub alpha: f64,
    pub is_onscreen: bool,
    pub is_frontmost: bool,
    pub bounds: WindowBounds,
}

pub(crate) fn parse_window_list_payload(
    payload: &str,
) -> Result<Vec<WindowCatalogEntry>, MacosError> {
    serde_json::from_str(payload)
        .map_err(|error| MacosError::Other(format!("failed to parse window list: {error}")))
}

/// Which window visibility states to include in the catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowScope {
    OnScreen,
    AllSpaces,
}

pub(crate) fn select_capturable_windows(
    entries: Vec<WindowCatalogEntry>,
    scope: WindowScope,
) -> Vec<CapturableWindow> {
    let mut windows = entries
        .into_iter()
        .filter(|entry| !entry.app.trim().is_empty())
        .filter(|entry| !entry.title.trim().is_empty())
        .filter(|entry| entry.bounds.width > 0 && entry.bounds.height > 0)
        .filter(|entry| scope == WindowScope::AllSpaces || entry.is_onscreen)
        .filter(|entry| entry.alpha > 0.0)
        .filter(|entry| entry.layer == 0)
        .filter(|entry| !is_noise_window(entry))
        .map(|entry| CapturableWindow {
            window_id: entry.window_id,
            app: entry.app,
            title: entry.title,
            owner_pid: entry.owner_pid,
            is_frontmost: entry.is_frontmost,
            is_onscreen: entry.is_onscreen,
            bounds: entry.bounds,
        })
        .collect::<Vec<_>>();

    windows.sort_by(|left, right| {
        right
            .is_frontmost
            .cmp(&left.is_frontmost)
            .then_with(|| {
                left.app
                    .to_ascii_lowercase()
                    .cmp(&right.app.to_ascii_lowercase())
            })
            .then_with(|| {
                left.title
                    .to_ascii_lowercase()
                    .cmp(&right.title.to_ascii_lowercase())
            })
            .then_with(|| left.window_id.cmp(&right.window_id))
    });

    windows
}

fn is_noise_window(entry: &WindowCatalogEntry) -> bool {
    matches!(
        (entry.app.as_str(), entry.title.as_str()),
        ("WindowManager", "App Icon Window") | ("WindowManager", "Gesture Blocking Overlay")
    )
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn find_capturable_window(
    windows: &[CapturableWindow],
    window_id: u32,
) -> Result<&CapturableWindow, MacosError> {
    windows
        .iter()
        .find(|window| window.window_id == window_id)
        .ok_or_else(|| MacosError::Other(format!("window id not found: {window_id}")))
}

/// List on-screen windows that can be captured, sorted with frontmost first.
pub fn list_capturable_windows() -> Result<Vec<CapturableWindow>, MacosError> {
    list_windows(WindowScope::OnScreen)
}

/// List titled application window candidates, optionally including non-visible windows.
/// Off-screen windows may belong to another Space, be hidden, or be minimized;
/// their presence in this catalog does not guarantee capture is available.
pub fn list_windows(scope: WindowScope) -> Result<Vec<CapturableWindow>, MacosError> {
    let script = include_str!("window_catalog.swift");

    let mut file = tempfile::NamedTempFile::with_suffix(".swift")
        .map_err(|error| MacosError::Other(format!("failed to create swift temp file: {error}")))?;
    file.write_all(script.as_bytes())
        .map_err(|error| MacosError::Other(format!("failed to write swift temp file: {error}")))?;

    let output = Command::new("swift")
        .arg(file.path())
        .arg(if scope == WindowScope::AllSpaces {
            "all"
        } else {
            "onscreen"
        })
        .output()
        .map_err(|error| MacosError::Other(format!("swift: {error}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MacosError::Other(format!(
            "failed to list windows: {stderr}"
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed = parse_window_list_payload(&stdout)?;
    Ok(select_capturable_windows(parsed, scope))
}
