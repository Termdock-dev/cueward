use chrono::{DateTime, Local};
use std::process::Command;

use crate::MacosError;

/// Escape a string for use in AppleScript double-quoted literals.
pub fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Escape a multi-line string for AppleScript by joining lines with `linefeed`.
pub fn escape_body(s: &str) -> String {
    let parts: Vec<String> = s
        .split('\n')
        .map(|line| format!("\"{}\"", escape(line)))
        .collect();
    parts.join(" & linefeed & ")
}

/// Build an AppleScript snippet that constructs a date object locale-independently.
pub fn applescript_date_block(var_name: &str, dt: &DateTime<Local>) -> String {
    format!(
        r#"set {var_name} to current date
            set day of {var_name} to 1
            set year of {var_name} to {y}
            set month of {var_name} to {m}
            set day of {var_name} to {d}
            set hours of {var_name} to {h}
            set minutes of {var_name} to {min}
            set seconds of {var_name} to {s}"#,
        var_name = var_name,
        y = dt.format("%Y"),
        m = dt.format("%-m"),
        d = dt.format("%-d"),
        h = dt.format("%-H"),
        min = dt.format("%-M"),
        s = dt.format("%-S"),
    )
}

/// Run an AppleScript and return its stdout on success.
pub fn run_capture(script: &str, context: &str) -> Result<String, MacosError> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|e| MacosError::Other(format!("osascript: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MacosError::Other(format!("{context}: {stderr}")));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Run an AppleScript and return Ok or a descriptive error.
pub fn run(script: &str, context: &str) -> Result<(), MacosError> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|e| MacosError::Other(format!("osascript: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MacosError::Other(format!("{context}: {stderr}")));
    }

    Ok(())
}
