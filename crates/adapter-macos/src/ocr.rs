use std::collections::HashMap;
use std::io::Write;
use std::process::Command;

use chrono::Utc;
use serde::Deserialize;

use cueward_core::{Cue, CueSource};

use crate::MacosError;

const OCR_SCRIPT: &str = include_str!("../scripts/ocr.swift");
const VISION_SCRIPT: &str = include_str!("../scripts/vision_text.swift");

#[derive(Debug, Deserialize)]
struct OcrResult {
    text: String,
    confidence: f32,
}

/// Run Vision OCR on an image or PDF file, returning Cues.
pub fn capture(path: &str) -> Result<Vec<Cue>, MacosError> {
    let abs_path = std::fs::canonicalize(path)
        .map_err(|_| MacosError::Other(format!("file not found: {path}")))?;

    let results = run_native(&abs_path)?;

    let text: String = results
        .iter()
        .filter(|r| r.confidence > 0.5)
        .map(|r| r.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    if text.is_empty() {
        return Ok(Vec::new());
    }

    let filename = abs_path
        .file_name()
        .map(|f| f.to_string_lossy().into_owned());

    Ok(vec![Cue {
        source: CueSource::Ocr,
        timestamp: Utc::now(),
        content: text,
        url: Some(format!("file://{}", abs_path.display())),
        title: filename,
        tags: Vec::new(),
        attachment_segments: Vec::new(),
        metadata: HashMap::from([("ocr".into(), "true".into())]),
    }])
}

fn run_native(path: &std::path::Path) -> Result<Vec<OcrResult>, MacosError> {
    // Write embedded script to temp file
    let mut tmp = tempfile::NamedTempFile::with_suffix(".swift")
        .map_err(|e| MacosError::Other(format!("failed to create temp file: {e}")))?;
    tmp.write_all(format!("{VISION_SCRIPT}\n{OCR_SCRIPT}").as_bytes())
        .map_err(|e| MacosError::Other(format!("failed to write script: {e}")))?;

    let output = Command::new("swift")
        .arg(tmp.path())
        .arg(path)
        .output()
        .map_err(|e| MacosError::Other(format!("swift: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MacosError::Other(format!("OCR failed: {stderr}")));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Err(MacosError::Other(
            "OCR produced no output (Swift encoder may have failed)".into(),
        ));
    }

    serde_json::from_str(&stdout)
        .map_err(|e| MacosError::Other(format!("failed to parse OCR output: {e}")))
}
