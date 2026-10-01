use super::*;
use serde::Deserialize;
use serde_json::json;
use std::process::Command;

#[derive(Debug, Deserialize)]
pub(super) struct NativePreview {
    #[serde(default)]
    pub total_pages: Option<usize>,
    #[serde(default)]
    pub pdf_encrypted: Option<bool>,
    #[serde(default)]
    pub image: Option<ImageInfo>,
    pub pages: Vec<PagePreview>,
    pub selection_truncated: bool,
}

/// Join the shared Vision recognizer and per-format native implementations.
pub(super) fn source() -> String {
    [
        include_str!("../../../scripts/vision_text.swift"),
        include_str!("types.swift"),
        include_str!("render.swift"),
        include_str!("pdf.swift"),
        include_str!("image.swift"),
        include_str!("thumbnail.swift"),
        include_str!("main.swift"),
    ]
    .join("\n")
}

/// Helper inherits the worker process group, so its compiler is stopped on timeout.
pub(super) fn inspect(
    input: &Path,
    directory: &Path,
    options: &PreviewOptions,
) -> Result<NativePreview, FileError> {
    let script = directory.join("preview.swift");
    let payload = directory.join("request.json");
    write_new(&script, source().as_bytes())?;
    write_new(
        &payload,
        &serde_json::to_vec(&json!({"input":input,"directory":directory,"options":options}))
            .map_err(internal)?,
    )?;
    let output = Command::new("swift")
        .arg(&script)
        .stdin(std::fs::File::open(&payload)?)
        .output()
        .map_err(|error| {
            FileError::new(
                FileErrorCode::Unavailable,
                format!("cannot start Swift preview helper: {error}"),
            )
        })?;
    std::fs::remove_file(script)?;
    std::fs::remove_file(payload)?;
    parse(&output)
}

/// Decode native errors and keep missing/broken toolchain distinct from source absence.
pub(super) fn parse(output: &std::process::Output) -> Result<native::NativePreview, FileError> {
    if !output.status.success() && output.stdout.is_empty() {
        return Err(FileError::new(
            FileErrorCode::Unavailable,
            format!(
                "Swift preview helper could not run: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        ));
    }
    if output.stdout.len() > 8 * 1024 * 1024 {
        return Err(internal("native preview response exceeds 8 MiB"));
    }
    let result: Result<NativePreview, FileError> =
        serde_json::from_slice(&output.stdout).map_err(|e| {
            internal(format!(
                "invalid native preview response: {e}; {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        })?;
    if result.is_ok() != output.status.success() {
        return Err(internal("native preview exit status disagrees with result"));
    }
    result
}

fn write_new(path: &Path, data: &[u8]) -> Result<(), FileError> {
    use std::io::Write;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(data)?;
    Ok(())
}
fn internal(message: impl std::fmt::Display) -> FileError {
    FileError::new(FileErrorCode::Internal, message.to_string())
}
