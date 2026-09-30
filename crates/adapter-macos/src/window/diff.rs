use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::process::run_with_timeout;
use super::{WindowSnapshot, snapshot_window};
use crate::MacosError;
use crate::screenshot::{CapturableWindow, WindowScope, list_windows};

/// Outcome of comparing one historical observation with the same window now.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotDiffStatus {
    Unchanged,
    ContentChanged,
    GeometryChanged,
    TargetChanged,
    WindowGone,
    ObservationFailed,
    NotComparable,
}

/// Exact differences after rendering both images into the same sRGB RGBA format.
#[derive(Debug, Serialize, Deserialize)]
pub struct SnapshotPixelDiff {
    pub changed_pixels: u64,
    pub total_pixels: u64,
    /// Bounding rectangle in current snapshot pixels, with a top-left origin.
    pub changed_bounds: Option<PixelBounds>,
}

/// Pixel rectangle in a snapshot image, excluding its shadow and attached sheets.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PixelBounds {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Difference evidence and a fresh full observation when capture succeeded.
#[derive(Debug, Serialize)]
pub struct SnapshotDiff {
    pub status: SnapshotDiffStatus,
    pub previous_observation_id: String,
    pub current_observation_id: Option<String>,
    pub previous_window: CapturableWindow,
    pub observed_window: Option<CapturableWindow>,
    /// None until the same target has been captured successfully.
    pub geometry_changed: Option<bool>,
    /// None when pixels could not be compared, including resized captures.
    pub content_changed: Option<bool>,
    pub coordinate_mapping_unchanged: bool,
    pub requires_reexploration: bool,
    pub pixels: Option<SnapshotPixelDiff>,
    pub reason: Option<String>,
    pub current: Option<WindowSnapshot>,
}

/// Read a saved snapshot and compare it with a fresh capture without sending input.
/// Historical tokens are never reused or renewed by this operation.
pub fn diff_window_snapshot(previous: &str) -> Result<SnapshotDiff, MacosError> {
    let previous = read_snapshot(Path::new(previous))?;
    Ok(diff_with(
        previous,
        || list_windows(WindowScope::AllSpaces),
        |id| snapshot_window(id, false, None),
        compare_pixels,
    ))
}

fn read_snapshot(path: &Path) -> Result<WindowSnapshot, MacosError> {
    let mut text = String::new();
    File::open(path)
        .and_then(|file| file.take(1_048_577).read_to_string(&mut text))
        .map_err(|error| MacosError::Other(format!("cannot read previous snapshot: {error}")))?;
    if text.len() > 1_048_576 {
        return Err(MacosError::Other("previous snapshot exceeds 1 MiB".into()));
    }
    parse_snapshot(&text)
}

fn parse_snapshot(text: &str) -> Result<WindowSnapshot, MacosError> {
    let text = text.trim();
    let payload =
        if let Some(body) = text.strip_prefix("<external source=\"cueward/window/snapshot\">\n") {
            body.strip_suffix("\n</external>")
                .ok_or_else(|| MacosError::Other("incomplete snapshot wrapper".into()))?
                .replace("&lt;/external&gt;", "</external>")
        } else {
            text.to_owned()
        };
    let snapshot: WindowSnapshot = serde_json::from_str(&payload)
        .map_err(|error| MacosError::Other(format!("invalid previous snapshot: {error}")))?;
    validate_snapshot(&snapshot)?;
    Ok(snapshot)
}

fn validate_snapshot(snapshot: &WindowSnapshot) -> Result<(), MacosError> {
    let bounds = snapshot.window.bounds;
    let image = &snapshot.image;
    if snapshot.window.window_id == 0
        || snapshot.window.owner_pid <= 0
        || bounds.width <= 0
        || bounds.height <= 0
        || image.width == 0
        || image.height == 0
        || image.origin != "window_frame_top_left"
        || image.scale_x != f64::from(image.width) / f64::from(bounds.width)
        || image.scale_y != f64::from(image.height) / f64::from(bounds.height)
    {
        return Err(MacosError::Other(
            "invalid previous snapshot coordinate metadata".into(),
        ));
    }
    Ok(())
}

fn observation_id(snapshot: &WindowSnapshot) -> String {
    // These serializable fields contain no custom serializer that can fail.
    let bytes = serde_json::to_vec(snapshot).unwrap_or_default();
    format!("{:x}", Sha256::digest(bytes))
}

fn diff_with(
    previous: WindowSnapshot,
    catalog: impl FnOnce() -> Result<Vec<CapturableWindow>, MacosError>,
    capture: impl FnOnce(u32) -> Result<WindowSnapshot, MacosError>,
    pixels: impl FnOnce(&WindowSnapshot, &WindowSnapshot) -> Result<SnapshotPixelDiff, MacosError>,
) -> SnapshotDiff {
    let mut result = empty_diff(&previous);
    let windows = match catalog() {
        Ok(windows) => windows,
        Err(error) => {
            result.reason = Some(error.to_string());
            return result;
        }
    };
    let Some(window) = windows
        .into_iter()
        .find(|w| w.window_id == previous.window.window_id)
    else {
        result.status = SnapshotDiffStatus::WindowGone;
        return result;
    };
    result.observed_window = Some(window.clone());
    if !same_target(&previous.window, &window) {
        result.status = SnapshotDiffStatus::TargetChanged;
        return result;
    }
    let current = match capture(window.window_id) {
        Ok(current) => current,
        Err(error) => {
            result.reason = Some(error.to_string());
            return result;
        }
    };
    result.current_observation_id = Some(observation_id(&current));
    result.observed_window = Some(current.window.clone());
    if !same_target(&previous.window, &current.window) {
        // The target can be replaced between the first catalog and capture.
        result.status = SnapshotDiffStatus::TargetChanged;
        return result;
    }
    compare_observations(&previous, &current, &mut result, pixels);
    result.current = Some(current);
    result
}

fn empty_diff(previous: &WindowSnapshot) -> SnapshotDiff {
    SnapshotDiff {
        status: SnapshotDiffStatus::ObservationFailed,
        previous_observation_id: observation_id(previous),
        current_observation_id: None,
        previous_window: previous.window.clone(),
        observed_window: None,
        geometry_changed: None,
        content_changed: None,
        coordinate_mapping_unchanged: false,
        requires_reexploration: true,
        pixels: None,
        reason: None,
        current: None,
    }
}

fn compare_observations(
    previous: &WindowSnapshot,
    current: &WindowSnapshot,
    result: &mut SnapshotDiff,
    pixels: impl FnOnce(&WindowSnapshot, &WindowSnapshot) -> Result<SnapshotPixelDiff, MacosError>,
) {
    let same_dimensions = previous.image.width == current.image.width
        && previous.image.height == current.image.height;
    let geometry_changed = previous.window.bounds != current.window.bounds
        || !same_dimensions
        || previous.image.scale_x != current.image.scale_x
        || previous.image.scale_y != current.image.scale_y
        || previous.image.origin != current.image.origin;
    result.geometry_changed = Some(geometry_changed);
    if !same_dimensions {
        result.status = SnapshotDiffStatus::GeometryChanged;
    } else {
        match pixels(previous, current) {
            Ok(pixels) => {
                let changed = pixels.changed_pixels != 0;
                result.content_changed = Some(changed);
                result.pixels = Some(pixels);
                result.coordinate_mapping_unchanged = !geometry_changed;
                result.status = if geometry_changed {
                    SnapshotDiffStatus::GeometryChanged
                } else if changed {
                    SnapshotDiffStatus::ContentChanged
                } else {
                    SnapshotDiffStatus::Unchanged
                };
                result.requires_reexploration = result.status != SnapshotDiffStatus::Unchanged;
            }
            Err(error) => {
                result.status = SnapshotDiffStatus::NotComparable;
                result.reason = Some(error.to_string());
            }
        }
    }
}

fn same_target(before: &CapturableWindow, after: &CapturableWindow) -> bool {
    before.window_id == after.window_id
        && before.owner_pid == after.owner_pid
        && before.title == after.title
        && before.app == after.app
}

fn compare_pixels(
    before: &WindowSnapshot,
    after: &WindowSnapshot,
) -> Result<SnapshotPixelDiff, MacosError> {
    let mut source = tempfile::NamedTempFile::with_suffix(".swift")
        .map_err(|error| MacosError::Other(format!("cannot prepare image comparison: {error}")))?;
    source
        .write_all(include_bytes!("diff_pixels.swift"))
        .map_err(|error| MacosError::Other(format!("cannot write image comparison: {error}")))?;
    let mut command = Command::new("swift");
    command
        .arg(source.path())
        .arg(&before.screenshot.path)
        .arg(&after.screenshot.path)
        .arg(before.image.width.to_string())
        .arg(before.image.height.to_string());
    let output = run_with_timeout(&mut command, &[], Duration::from_secs(40))
        .map_err(|error| MacosError::Other(format!("image comparison failed: {error}")))?;
    if !output.status.success() {
        return Err(MacosError::Other(format!(
            "image comparison failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| MacosError::Other(format!("invalid image comparison result: {error}")))
}

#[cfg(test)]
#[path = "diff_tests.rs"]
mod tests;
