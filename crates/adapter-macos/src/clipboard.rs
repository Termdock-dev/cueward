use std::fs;
use std::path::{Component, Path};
use std::process::Command;

use chrono::Local;
use serde::Serialize;

use crate::MacosError;

#[derive(Debug, Serialize)]
pub struct ClipboardContent {
    #[serde(rename = "type")]
    pub content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

const CACHE_DIR: &str = ".cueward/cache/clipboard";

fn validate_user_output_path(path: &str) -> Result<&Path, String> {
    let candidate = Path::new(path);
    if candidate
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err("path must not contain parent directory components".into());
    }
    Ok(candidate)
}

fn ensure_cache_dir() -> Result<String, MacosError> {
    let home = std::env::var("HOME").map_err(|_| MacosError::Other("HOME not set".into()))?;
    let dir = format!("{home}/{CACHE_DIR}");
    fs::create_dir_all(&dir)
        .map_err(|e| MacosError::Other(format!("failed to create {dir}: {e}")))?;
    Ok(dir)
}

/// Check if clipboard contains an image.
fn has_image() -> bool {
    let output = Command::new("osascript")
        .arg("-e")
        .arg("clipboard info")
        .output();
    match output {
        Ok(o) => {
            let info = String::from_utf8_lossy(&o.stdout);
            info.contains("«class PNGf»") || info.contains("«class TIFF»")
        }
        Err(_) => false,
    }
}

/// Save clipboard image to PNG using AppleScript with Cocoa bridge.
fn save_clipboard_image(save_path: &str) -> Result<(), MacosError> {
    validate_user_output_path(save_path).map_err(MacosError::Other)?;

    let script = format!(
        r#"
        use framework "AppKit"
        set pb to current application's NSPasteboard's generalPasteboard()
        set imgData to pb's dataForType:(current application's NSPasteboardTypePNG)
        if imgData is missing value then
            set tiffData to pb's dataForType:(current application's NSPasteboardTypeTIFF)
            if tiffData is missing value then error "no image in clipboard"
            set bitmapRep to current application's NSBitmapImageRep's imageRepWithData:tiffData
            set imgData to bitmapRep's representationUsingType:(current application's NSBitmapImageFileTypePNG) properties:(missing value)
        end if
        imgData's writeToFile:"{save_path}" atomically:true
        "#,
        save_path = save_path.replace('\\', "\\\\").replace('"', "\\\""),
    );

    let output = Command::new("osascript")
        .arg("-l")
        .arg("AppleScript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| MacosError::Other(format!("osascript: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MacosError::Other(format!(
            "failed to save clipboard image: {stderr}"
        )));
    }

    Ok(())
}

/// Read clipboard content. Returns text or saves image and returns path.
pub fn get(save_image_path: Option<&str>) -> Result<ClipboardContent, MacosError> {
    if has_image() {
        let path = match save_image_path {
            Some(p) => p.to_string(),
            None => {
                let dir = ensure_cache_dir()?;
                let ts = Local::now().format("%Y%m%d-%H%M%S").to_string();
                format!("{dir}/{ts}.png")
            }
        };

        save_clipboard_image(&path)?;

        return Ok(ClipboardContent {
            content_type: "image".into(),
            content: None,
            path: Some(path),
        });
    }

    let output = Command::new("pbpaste")
        .output()
        .map_err(|e| MacosError::Other(format!("pbpaste: {e}")))?;

    let text = String::from_utf8_lossy(&output.stdout).into_owned();

    Ok(ClipboardContent {
        content_type: "text".into(),
        content: Some(text),
        path: None,
    })
}

/// Write text to clipboard via pbcopy (stdin pipe, no shell injection).
pub fn set(text: &str) -> Result<(), MacosError> {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = Command::new("pbcopy")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| MacosError::Other(format!("pbcopy: {e}")))?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| MacosError::Other("failed to open stdin for pbcopy".into()))?;
    stdin
        .write_all(text.as_bytes())
        .map_err(|e| MacosError::Other(format!("failed to write to pbcopy: {e}")))?;

    let status = child
        .wait()
        .map_err(|e| MacosError::Other(format!("pbcopy: {e}")))?;

    if !status.success() {
        return Err(MacosError::Other("pbcopy failed".into()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::validate_user_output_path;

    #[test]
    fn validate_user_output_path_rejects_parent_components() {
        let err = validate_user_output_path("../../etc/passwd").expect_err("should reject");

        assert!(err.contains("parent directory"));
    }

    #[test]
    fn validate_user_output_path_accepts_normal_relative_paths() {
        let path = validate_user_output_path("screenshots/out.png").expect("path");

        assert_eq!(path, Path::new("screenshots/out.png"));
    }
}
