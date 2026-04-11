use std::fs;
use std::path::{Component, Path};
use std::process::Command;

use chrono::Local;
use serde::Serialize;

use crate::MacosError;

#[derive(Debug, Serialize)]
pub struct ScreenshotResult {
    pub path: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ocr_text: Option<String>,
}

const CACHE_DIR: &str = ".cueward/cache/screenshots";

fn validate_user_output_path(path: &str) -> Result<&Path, String> {
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

fn validate_display(display: Option<u32>) -> Result<(), String> {
    if let Some(display) = display {
        if !(1..=10).contains(&display) {
            return Err(format!(
                "invalid display number {display}: must be between 1 and 10"
            ));
        }
    }
    Ok(())
}

fn ensure_screenshot_file_exists(path: &Path) -> Result<(), String> {
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

/// Capture a screenshot.
/// `display`: None = main screen, Some(n) = display number (1 = main, 2 = secondary, 3 = third).
/// `ocr`: run Vision OCR on the captured image.
/// `output`: save to this path instead of cache dir.
pub fn capture(
    ocr: bool,
    output: Option<&str>,
    display: Option<u32>,
) -> Result<ScreenshotResult, MacosError> {
    validate_display(display).map_err(MacosError::Other)?;

    let now = Local::now();
    let timestamp = now.format("%Y%m%d-%H%M%S").to_string();

    let path = match output {
        Some(p) => {
            validate_user_output_path(p).map_err(MacosError::Other)?;
            p.to_string()
        }
        None => {
            let dir = ensure_cache_dir()?;
            let suffix = display.map(|d| format!("-d{d}")).unwrap_or_default();
            format!("{dir}/{timestamp}{suffix}.png")
        }
    };

    // -x = silent (no shutter sound), -D = display number
    let mut cmd = Command::new("screencapture");
    cmd.arg("-x");
    if let Some(d) = display {
        cmd.args(["-D", &d.to_string()]);
    }
    cmd.arg(&path);
    let status = cmd
        .status()
        .map_err(|e| MacosError::Other(format!("screencapture: {e}")))?;

    if !status.success() {
        return Err(MacosError::Other("screencapture failed".into()));
    }

    ensure_screenshot_file_exists(Path::new(&path)).map_err(MacosError::Other)?;

    let ocr_text = if ocr {
        match crate::ocr::capture(&path) {
            Ok(cues) => {
                let text: String = cues
                    .into_iter()
                    .map(|c| c.content)
                    .collect::<Vec<_>>()
                    .join("\n");
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{ensure_screenshot_file_exists, validate_display, validate_user_output_path};

    #[test]
    fn validate_user_output_path_rejects_parent_components() {
        let err = validate_user_output_path("../shot.png").expect_err("should reject");

        assert!(err.contains("parent directory"));
    }

    #[test]
    fn ensure_screenshot_file_exists_reports_missing_file() {
        let err = ensure_screenshot_file_exists(Path::new("/tmp/does-not-exist-cueward.png"))
            .expect_err("should fail");

        assert!(err.contains("was not created"));
    }

    #[test]
    fn validate_display_rejects_out_of_range_values() {
        let err = validate_display(Some(11)).expect_err("should reject");

        assert!(err.contains("between 1 and 10"));
    }
}
