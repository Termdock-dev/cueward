use super::bridge::run_ax;
use super::target::{Target, WindowIdentity, now_seconds};
use super::{AccessibilitySnapshot, WindowInspectResult};
use crate::MacosError;
use crate::screenshot::{CapturableWindow, capture_window, list_capturable_windows};

fn root_depth(root: &str) -> Result<usize, MacosError> {
    let parts: Vec<_> = root.split('.').collect();
    if parts.first() != Some(&"0")
        || parts.len() > 13
        || parts.iter().any(|part| {
            part.is_empty()
                || !part.bytes().all(|byte| byte.is_ascii_digit())
                || (part.len() > 1 && part.starts_with('0'))
                || part.parse::<isize>().is_err()
        })
    {
        return Err(MacosError::Other(
            "window root must be an absolute element ref such as 0.1.2, at most 12 levels deep"
                .into(),
        ));
    }
    Ok(parts.len() - 1)
}

fn run_ax_inspect(
    window: &CapturableWindow,
    root: &str,
    limit: usize,
    max_depth: usize,
) -> Result<AccessibilitySnapshot, MacosError> {
    let identity = WindowIdentity::from(window);
    let issued_at = now_seconds()?;
    let mut snapshot: AccessibilitySnapshot = run_ax(
        &identity,
        include_str!("ax_inspect.swift"),
        &[limit.to_string(), max_depth.to_string(), root.to_string()],
        &serde_json::Value::Null,
    )?;
    if snapshot.window_id != window.window_id
        || snapshot.owner_pid != window.owner_pid
        || snapshot.root_ref != root
    {
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

/// Inspect an exact on-screen window through Accessibility, with optional screenshot and OCR.
pub fn inspect_window(
    window_id: u32,
    limit: usize,
    max_depth: usize,
    screenshot: bool,
    ocr: bool,
) -> Result<WindowInspectResult, MacosError> {
    inspect_window_subtree(window_id, "0", limit, max_depth, screenshot, ocr)
}

/// Read a subtree at a current absolute element ref; returned action tokens bind fresh identities.
pub fn inspect_window_subtree(
    window_id: u32,
    root: &str,
    limit: usize,
    max_depth: usize,
    screenshot: bool,
    ocr: bool,
) -> Result<WindowInspectResult, MacosError> {
    let depth = root_depth(root)?;
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
    let accessibility = run_ax_inspect(&window, root, limit, max_depth.min(12 - depth))?;
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

#[cfg(test)]
#[path = "inspection_tests.rs"]
mod tests;
