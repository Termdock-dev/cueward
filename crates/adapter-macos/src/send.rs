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

    run_osascript(&script, "failed to create note")
}

/// Update an existing note's body by title.
pub fn update_note(title: &str, body: &str, folder: &str) -> Result<(), MacosError> {
    let escaped_title = title.replace('"', "\\\"");
    let escaped_body = body.replace('"', "\\\"").replace('\n', "\\n");

    let script = format!(
        r#"
        tell application "Notes"
            set theNote to (first note of folder "{folder}" whose name is "{escaped_title}")
            set body of theNote to "{escaped_body}"
        end tell
        "#
    );

    run_osascript(&script, "failed to update note")
}

/// Delete a note by title from a specific folder.
pub fn delete_note(title: &str, folder: &str) -> Result<(), MacosError> {
    let escaped_title = title.replace('"', "\\\"");

    let script = format!(
        r#"
        tell application "Notes"
            delete (first note of folder "{folder}" whose name is "{escaped_title}")
        end tell
        "#
    );

    run_osascript(&script, "failed to delete note")
}

/// Move a note to a different folder.
pub fn move_note(title: &str, from_folder: &str, to_folder: &str) -> Result<(), MacosError> {
    let escaped_title = title.replace('"', "\\\"");

    let script = format!(
        r#"
        tell application "Notes"
            try
                set destFolder to folder "{to_folder}"
            on error
                make new folder with properties {{name:"{to_folder}"}}
                set destFolder to folder "{to_folder}"
            end try
            set theNote to (first note of folder "{from_folder}" whose name is "{escaped_title}")
            move theNote to destFolder
        end tell
        "#
    );

    run_osascript(&script, "failed to move note")
}

fn run_osascript(script: &str, context: &str) -> Result<(), MacosError> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|e| MacosError::PermissionDenied(format!("osascript: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MacosError::PermissionDenied(format!(
            "{context}: {stderr}"
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

    run_osascript(&script, "notification failed")
}
