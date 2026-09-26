use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::input_target::InputTarget;
use super::target::WindowIdentity;
use crate::MacosError;
use crate::screenshot::{
    CapturableWindow, ScreenshotResult, WindowBounds, WindowScope, capture_window_frame,
    ensure_cache_dir, list_windows, validate_user_output_path,
};

#[derive(Debug, Serialize)]
pub struct SnapshotImage {
    pub width: u32,
    pub height: u32,
    /// Image pixels per window-frame point on each axis.
    pub scale_x: f64,
    pub scale_y: f64,
    pub origin: &'static str,
}

#[derive(Debug, Serialize)]
pub struct WindowSnapshot {
    pub window: CapturableWindow,
    pub screenshot: ScreenshotResult,
    pub image: SnapshotImage,
    pub input_target: String,
}

/// Capture an exact window, including other Spaces, without activating its app.
/// The image excludes the shadow and maps to the window frame's top-left origin.
pub fn snapshot_window(
    window_id: u32,
    ocr: bool,
    output: Option<&str>,
) -> Result<WindowSnapshot, MacosError> {
    snapshot_with(
        window_id,
        ocr,
        output,
        || list_windows(WindowScope::AllSpaces),
        capture_window_frame,
    )
}

fn snapshot_with(
    window_id: u32,
    ocr: bool,
    output: Option<&str>,
    mut catalog: impl FnMut() -> Result<Vec<CapturableWindow>, MacosError>,
    capture: impl FnOnce(bool, &str, u32) -> Result<ScreenshotResult, MacosError>,
) -> Result<WindowSnapshot, MacosError> {
    if let Some(path) = output {
        validate_user_output_path(path).map_err(MacosError::Other)?;
    }
    let before = find_window(catalog()?, window_id)?;
    let destination = match output {
        Some(path) => PathBuf::from(path),
        None => Path::new(&ensure_cache_dir()?)
            .join(format!("window-{window_id}-{}.png", uuid::Uuid::new_v4())),
    };
    let directory = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let temporary = tempfile::Builder::new()
        .prefix("cueward-window-")
        .suffix(".png")
        .tempfile_in(directory)
        .map_err(|error| MacosError::Other(format!("failed to prepare snapshot: {error}")))?;
    let temporary_path = temporary
        .path()
        .to_str()
        .ok_or_else(|| MacosError::Other("snapshot path must be valid UTF-8".into()))?;
    let mut screenshot = capture(ocr, temporary_path, window_id)?;
    let image = read_image_metadata(temporary.path(), before.bounds)?;
    let after = find_window(catalog()?, window_id)?;
    if WindowIdentity::from(&before) != WindowIdentity::from(&after) {
        return Err(MacosError::Other(
            "window changed during capture; take a new snapshot".into(),
        ));
    }
    let input_target = InputTarget::issue(&after, &image)?;
    temporary
        .persist(&destination)
        .map_err(|error| MacosError::Other(format!("failed to save snapshot: {}", error.error)))?;
    screenshot.path = destination.to_string_lossy().into_owned();
    Ok(WindowSnapshot {
        window: after,
        screenshot,
        image,
        input_target,
    })
}

fn find_window(windows: Vec<CapturableWindow>, id: u32) -> Result<CapturableWindow, MacosError> {
    windows
        .into_iter()
        .find(|window| window.window_id == id)
        .ok_or_else(|| MacosError::NotFound(format!("window id not found: {id}")))
}

fn read_image_metadata(path: &Path, bounds: WindowBounds) -> Result<SnapshotImage, MacosError> {
    let mut header = [0u8; 24];
    File::open(path)
        .and_then(|mut file| file.read_exact(&mut header))
        .map_err(|error| {
            MacosError::Other(format!("failed to read snapshot dimensions: {error}"))
        })?;
    // Read the fixed PNG signature and IHDR dimensions, not an image decoder.
    if &header[..8] != b"\x89PNG\r\n\x1a\n"
        || header[8..12] != 13u32.to_be_bytes()
        || &header[12..16] != b"IHDR"
    {
        return Err(MacosError::Other("snapshot is not a PNG image".into()));
    }
    let width = u32::from_be_bytes([header[16], header[17], header[18], header[19]]);
    let height = u32::from_be_bytes([header[20], header[21], header[22], header[23]]);
    if width == 0
        || height == 0
        || width > i32::MAX as u32
        || height > i32::MAX as u32
        || bounds.width <= 0
        || bounds.height <= 0
    {
        return Err(MacosError::Other("invalid snapshot dimensions".into()));
    }
    Ok(SnapshotImage {
        width,
        height,
        scale_x: f64::from(width) / f64::from(bounds.width),
        scale_y: f64::from(height) / f64::from(bounds.height),
        origin: "window_frame_top_left",
    })
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;
