use std::collections::HashMap;
use std::process::Command;

use chrono::{DateTime, TimeZone, Utc};

use cueward_core::{Cue, CueSource};

use crate::MacosError;

pub fn capture(since: DateTime<Utc>) -> Result<Vec<Cue>, MacosError> {
    let seconds_ago = (Utc::now() - since).num_seconds().max(0);

    // Compute unix timestamps in AppleScript by getting the current unix time via
    // `date +%s` and subtracting the delta between `current date` and `modification date`.
    // This avoids locale-dependent date formatting and timezone offset issues.
    let script = format!(
        r#"
        set output to ""
        set sinceDate to (current date) - {seconds_ago}
        set nowDate to current date
        tell application "Notes"
            set allNotes to every note
            repeat with theNote in allNotes
                try
                    set modDate to modification date of theNote
                    if modDate > sinceDate then
                        set noteName to name of theNote
                        set noteBody to plaintext of theNote
                        try
                            set theContainer to container of theNote
                            set noteFolder to name of theContainer
                        on error
                            set noteFolder to "Unknown"
                        end try
                        set secsDelta to nowDate - modDate
                        set unixStr to do shell script "echo $(( $(date +%s) - " & secsDelta & " ))"
                        set output to output & "---CUE_SEP---" & unixStr & "---FIELD---" & noteName & "---FIELD---" & noteFolder & "---FIELD---" & noteBody
                    end if
                end try
            end repeat
        end tell
        return output
        "#
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| MacosError::PermissionDenied(format!("osascript: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not allowed") || stderr.contains("denied") {
            return Err(MacosError::PermissionDenied(
                "Apple Notes access denied. Allow automation in System Settings > Privacy & Security > Automation".into(),
            ));
        }
        return Err(MacosError::PermissionDenied(format!("osascript error: {stderr}")));
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    let cues = raw
        .split("---CUE_SEP---")
        .filter(|s| !s.trim().is_empty())
        .filter_map(|entry| {
            let fields: Vec<&str> = entry.splitn(4, "---FIELD---").collect();
            if fields.len() < 4 {
                return None;
            }
            let unix_ts: i64 = fields[0].trim().parse().ok()?;
            let timestamp = Utc.timestamp_opt(unix_ts, 0).single()?;
            let title = fields[1].trim().to_string();
            let folder = fields[2].trim().to_string();
            let body = fields[3].trim().to_string();

            Some(Cue {
                source: CueSource::Notes,
                timestamp,
                content: body,
                url: None,
                title: Some(title),
                tags: Vec::new(),
                metadata: HashMap::from([("folder".into(), folder)]),
            })
        })
        .collect();

    Ok(cues)
}
