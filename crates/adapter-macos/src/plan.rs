use std::process::Command;

use crate::MacosError;

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
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

/// Create a reminder in Apple Reminders.
pub fn create_reminder(title: &str, notes: &str, list: &str) -> Result<(), MacosError> {
    let escaped_title = escape(title);
    let escaped_notes = escape(notes);
    let escaped_list = escape(list);

    let script = format!(
        r#"
        tell application "Reminders"
            try
                set targetList to list "{escaped_list}"
            on error
                make new list with properties {{name:"{escaped_list}"}}
                set targetList to list "{escaped_list}"
            end try
            make new reminder at targetList with properties {{name:"{escaped_title}", body:"{escaped_notes}"}}
        end tell
        "#
    );

    run_osascript(&script, "failed to create reminder")
}
