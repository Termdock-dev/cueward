use std::process::Command;

use crate::MacosError;

/// Create a note in Apple Notes with the given title and body.
pub fn create_note(title: &str, body: &str, folder: &str) -> Result<(), MacosError> {
    let escaped_title = title.replace('"', "\\\"");
    let escaped_body = body.replace('"', "\\\"").replace('\n', "\\n");

    let script = format!(
        r#"
        tell application "Notes"
            try
                set targetFolder to folder "{folder}"
            on error
                make new folder with properties {{name:"{folder}"}}
                set targetFolder to folder "{folder}"
            end try
            make new note at targetFolder with properties {{name:"{escaped_title}", body:"{escaped_body}"}}
        end tell
        "#
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| MacosError::PermissionDenied(format!("osascript: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MacosError::PermissionDenied(format!(
            "failed to create note: {stderr}"
        )));
    }

    Ok(())
}

/// Send a macOS notification via osascript.
pub fn notify(title: &str, message: &str) -> Result<(), MacosError> {
    let escaped_title = title.replace('"', "\\\"");
    let escaped_msg = message.replace('"', "\\\"");

    let script = format!(
        r#"display notification "{escaped_msg}" with title "{escaped_title}""#
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| MacosError::PermissionDenied(format!("osascript: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MacosError::PermissionDenied(format!(
            "notification failed: {stderr}"
        )));
    }

    Ok(())
}
