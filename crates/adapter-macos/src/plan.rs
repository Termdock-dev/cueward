use std::process::Command;

use crate::MacosError;

/// Create a reminder in Apple Reminders.
pub fn create_reminder(title: &str, notes: &str, list: &str) -> Result<(), MacosError> {
    let escaped_title = title.replace('"', "\\\"");
    let escaped_notes = notes.replace('"', "\\\"");

    let script = format!(
        r#"
        tell application "Reminders"
            try
                set targetList to list "{list}"
            on error
                make new list with properties {{name:"{list}"}}
                set targetList to list "{list}"
            end try
            make new reminder at targetList with properties {{name:"{escaped_title}", body:"{escaped_notes}"}}
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
            "failed to create reminder: {stderr}"
        )));
    }

    Ok(())
}

/// Create a calendar event via AppleScript.
pub fn create_calendar_event(
    title: &str,
    start_date: &str,
    end_date: &str,
    calendar: &str,
) -> Result<(), MacosError> {
    let escaped_title = title.replace('"', "\\\"");

    // AppleScript date format: "April 7, 2026 at 10:00:00 AM"
    let script = format!(
        r#"
        tell application "Calendar"
            tell calendar "{calendar}"
                make new event with properties {{summary:"{escaped_title}", start date:(date "{start_date}"), end date:(date "{end_date}")}}
            end tell
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
            "failed to create calendar event: {stderr}"
        )));
    }

    Ok(())
}
