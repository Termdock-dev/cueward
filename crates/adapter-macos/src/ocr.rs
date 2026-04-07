use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use chrono::Utc;
use serde::Deserialize;

use cueward_core::{Cue, CueSource};

use crate::MacosError;

#[derive(Debug, Deserialize)]
struct OcrResult {
    text: String,
    confidence: f32,
}

/// Run Vision OCR on an image or PDF file, returning Cues.
pub fn capture(path: &str) -> Result<Vec<Cue>, MacosError> {
    if !Path::new(path).exists() {
        return Err(MacosError::PermissionDenied(format!(
            "file not found: {path}"
        )));
    }

    // Find the ocr.swift script relative to the binary or fall back to common locations
    let script = find_ocr_script()?;

    let output = Command::new("swift")
        .arg(&script)
        .arg(path)
        .output()
        .map_err(|e| MacosError::PermissionDenied(format!("swift: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MacosError::PermissionDenied(format!("OCR failed: {stderr}")));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results: Vec<OcrResult> = serde_json::from_str(&stdout)
        .map_err(|e| MacosError::PermissionDenied(format!("failed to parse OCR output: {e}")))?;

    let text: String = results
        .iter()
        .filter(|r| r.confidence > 0.5)
        .map(|r| r.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    if text.is_empty() {
        return Ok(Vec::new());
    }

    let filename = Path::new(path)
        .file_name()
        .map(|f| f.to_string_lossy().into_owned());

    Ok(vec![Cue {
        source: CueSource::Safari, // Reuse safari as generic source for now
        timestamp: Utc::now(),
        content: text,
        url: Some(format!("file://{path}")),
        title: filename,
        tags: Vec::new(),
        metadata: HashMap::from([("ocr".into(), "true".into())]),
    }])
}

fn find_ocr_script() -> Result<String, MacosError> {
    // Check common locations
    let candidates = [
        // Relative to the repo
        "crates/adapter-macos/scripts/ocr.swift",
        // Installed location
        "/usr/local/share/cueward/ocr.swift",
    ];

    for path in &candidates {
        if Path::new(path).exists() {
            return Ok(path.to_string());
        }
    }

    // Try to find via the binary's location
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let script = dir.join("ocr.swift");
            if script.exists() {
                return Ok(script.to_string_lossy().into_owned());
            }
        }
    }

    Err(MacosError::PermissionDenied(
        "ocr.swift not found. Ensure it's in the scripts/ directory or /usr/local/share/cueward/"
            .into(),
    ))
}
