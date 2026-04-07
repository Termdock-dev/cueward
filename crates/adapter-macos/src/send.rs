use std::process::Command;

use crate::MacosError;

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn escape_body(s: &str) -> String {
    // AppleScript doesn't support \n in strings.
    // Split on newlines and join with `& linefeed &`.
    let parts: Vec<String> = s.split('\n').map(|line| {
        format!("\"{}\"", escape(line))
    }).collect();
    parts.join(" & linefeed & ")
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

/// Create a note in Apple Notes with the given title and body.
pub fn create_note(title: &str, body: &str, folder: &str) -> Result<(), MacosError> {
    let escaped_title = escape(title);
    let escaped_folder = escape(folder);
    let body_expr = escape_body(body);

    let script = format!(
        r#"
        tell application "Notes"
            try
                set targetFolder to folder "{escaped_folder}"
            on error
                make new folder with properties {{name:"{escaped_folder}"}}
                set targetFolder to folder "{escaped_folder}"
            end try
            make new note at targetFolder with properties {{name:"{escaped_title}", body:{body_expr}}}
        end tell
        "#
    );

    run_osascript(&script, "failed to create note")
}

/// Update an existing note's body by title.
pub fn update_note(title: &str, body: &str, folder: &str) -> Result<(), MacosError> {
    let escaped_title = escape(title);
    let escaped_folder = escape(folder);
    let body_expr = escape_body(body);

    let script = format!(
        r#"
        tell application "Notes"
            set theNote to (first note of folder "{escaped_folder}" whose name is "{escaped_title}")
            set body of theNote to {body_expr}
        end tell
        "#
    );

    run_osascript(&script, "failed to update note")
}

/// Delete a note by title from a specific folder.
pub fn delete_note(title: &str, folder: &str) -> Result<(), MacosError> {
    let escaped_title = escape(title);
    let escaped_folder = escape(folder);

    let script = format!(
        r#"
        tell application "Notes"
            delete (first note of folder "{escaped_folder}" whose name is "{escaped_title}")
        end tell
        "#
    );

    run_osascript(&script, "failed to delete note")
}

/// Move a note to a different folder.
pub fn move_note(title: &str, from_folder: &str, to_folder: &str) -> Result<(), MacosError> {
    let escaped_title = escape(title);
    let escaped_from = escape(from_folder);
    let escaped_to = escape(to_folder);

    let script = format!(
        r#"
        tell application "Notes"
            try
                set destFolder to folder "{escaped_to}"
            on error
                make new folder with properties {{name:"{escaped_to}"}}
                set destFolder to folder "{escaped_to}"
            end try
            set theNote to (first note of folder "{escaped_from}" whose name is "{escaped_title}")
            move theNote to destFolder
        end tell
        "#
    );

    run_osascript(&script, "failed to move note")
}

/// Send a macOS notification via osascript.
pub fn notify(title: &str, message: &str) -> Result<(), MacosError> {
    let escaped_title = escape(title);
    let escaped_msg = escape(message);

    let script = format!(
        r#"display notification "{escaped_msg}" with title "{escaped_title}""#
    );

    run_osascript(&script, "notification failed")
}
