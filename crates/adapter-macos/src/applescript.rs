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
