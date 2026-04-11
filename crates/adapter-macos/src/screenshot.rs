use std::fs;
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

fn ensure_cache_dir() -> Result<String, MacosError> {
    let home = std::env::var("HOME")
        .map_err(|_| MacosError::Other("HOME not set".into()))?;
    let dir = format!("{home}/{CACHE_DIR}");
    fs::create_dir_all(&dir)
        .map_err(|e| MacosError::Other(format!("failed to create {dir}: {e}")))?;
    Ok(dir)
}

/// Capture a screenshot of the entire screen.
/// If `ocr` is true, runs Vision OCR on the image.
/// If `output` is Some, saves to that path instead of cache dir.
pub fn capture(ocr: bool, output: Option<&str>) -> Result<ScreenshotResult, MacosError> {
    let now = Local::now();
    let timestamp = now.format("%Y%m%d-%H%M%S").to_string();

    let path = match output {
        Some(p) => p.to_string(),
        None => {
            let dir = ensure_cache_dir()?;
            format!("{dir}/{timestamp}.png")
        }
    };

    // -x = silent (no shutter sound)
    let status = Command::new("screencapture")
        .args(["-x", &path])
        .status()
        .map_err(|e| MacosError::Other(format!("screencapture: {e}")))?;

    if !status.success() {
        return Err(MacosError::Other("screencapture failed".into()));
    }

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
