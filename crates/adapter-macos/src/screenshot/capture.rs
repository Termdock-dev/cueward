use std::fs;
use std::path::{Component, Path};
use std::process::Command;

use chrono::Local;
use serde::Serialize;

use crate::MacosError;

const CACHE_DIR: &str = ".cueward/cache/screenshots";

#[derive(Debug, Serialize)]
pub struct ScreenshotResult {
    pub path: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ocr_text: Option<String>,
}

/// Validate that a user-supplied output path contains no control characters or parent directory components.
pub fn validate_user_output_path(path: &str) -> Result<&Path, String> {
    let candidate = Path::new(path);
    if path.contains('\n') || path.contains('\r') {
        return Err("path must not contain control characters".into());
    }
    if candidate
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err("path must not contain parent directory components".into());
    }
    Ok(candidate)
}

/// Validate that a display number, if given, is within the allowed range (1..=10).
pub fn validate_display(display: Option<u32>) -> Result<(), String> {
    if let Some(display) = display {
        if !(1..=10).contains(&display) {
            return Err(format!(
                "invalid display number {display}: must be between 1 and 10"
            ));
        }
    }
    Ok(())
}

/// Check that a screenshot file was created at the expected path.
pub fn ensure_screenshot_file_exists(path: &Path) -> Result<(), String> {
    if path.exists() {
        Ok(())
    } else {
        Err(format!(
            "screenshot file was not created at {}",
            path.display()
        ))
    }
}

fn ensure_cache_dir() -> Result<String, MacosError> {
    let home = std::env::var("HOME").map_err(|_| MacosError::Other("HOME not set".into()))?;
    let dir = format!("{home}/{CACHE_DIR}");
    fs::create_dir_all(&dir)
        .map_err(|e| MacosError::Other(format!("failed to create {dir}: {e}")))?;
    Ok(dir)
}

fn capture_to_path(
    ocr: bool,
    output: Option<&str>,
    suffix: &str,
    configure: impl FnOnce(&mut Command),
) -> Result<ScreenshotResult, MacosError> {
    let now = Local::now();
    let timestamp = now.format("%Y%m%d-%H%M%S").to_string();

    let path = match output {
        Some(p) => {
            validate_user_output_path(p).map_err(MacosError::Other)?;
            p.to_string()
        }
        None => {
            let dir = ensure_cache_dir()?;
            format!("{dir}/{timestamp}{suffix}.png")
        }
    };

    let mut cmd = Command::new("screencapture");
    cmd.arg("-x");
    configure(&mut cmd);
    cmd.arg(&path);
    let output_result = cmd
        .output()
        .map_err(|e| MacosError::Other(format!("screencapture: {e}")))?;

    if !output_result.status.success() {
        let stderr = String::from_utf8_lossy(&output_result.stderr);
        if stderr.trim().is_empty() {
            return Err(MacosError::Other("screencapture failed".into()));
        }
        return Err(MacosError::Other(format!(
            "screencapture failed: {}",
            stderr.trim()
        )));
    }

    ensure_screenshot_file_exists(Path::new(&path)).map_err(MacosError::Other)?;

    let ocr_text = if ocr {
        match crate::ocr::capture(&path) {
            Ok(cues) => {
                let text = cues.into_iter().map(|c| c.content).collect::<Vec<_>>().join("\n");
                if text.is_empty() { None } else { Some(text) }
            }
            Err(e) => {
                eprintln!("warning: OCR failed: {e}");
                None
            }
        }
    } else {
        None
    };

    Ok(ScreenshotResult {
        path,
        timestamp: now.to_rfc3339(),
        ocr_text,
    })
}

/// Capture a full-screen screenshot.
pub fn capture(
    ocr: bool,
    output: Option<&str>,
    display: Option<u32>,
) -> Result<ScreenshotResult, MacosError> {
    validate_display(display).map_err(MacosError::Other)?;

    let suffix = display.map(|d| format!("-d{d}")).unwrap_or_default();
    capture_to_path(ocr, output, &suffix, |cmd| {
        if let Some(d) = display {
            cmd.args(["-D", &d.to_string()]);
        }
    })
}

/// Capture a specific window by id.
pub fn capture_window(
    ocr: bool,
    output: Option<&str>,
    window_id: u32,
) -> Result<ScreenshotResult, MacosError> {
    capture_to_path(ocr, output, &format!("-w{window_id}"), |cmd| {
        cmd.args(["-l", &window_id.to_string()]);
    })
}
