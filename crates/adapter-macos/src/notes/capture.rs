use std::collections::HashMap;
use std::process::Command;

use chrono::{DateTime, TimeZone, Utc};
use cueward_core::{Cue, CueSource};

use crate::MacosError;

use super::attachments::{attachment_placeholder_count, enrich_cues_with_attachments};
use super::db::{load_map_notes, load_media_notes, load_web_preview_notes};
use super::{ATTACHMENT_LABEL, ATTACHMENT_PLACEHOLDER};

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
        return Err(MacosError::PermissionDenied(format!(
            "osascript error: {stderr}"
        )));
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    let mut cues: Vec<Cue> = raw
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
            let (body, _) = normalize_plaintext(fields[3].trim());

            let metadata = HashMap::from([("folder".into(), folder)]);

            Some(Cue {
                source: CueSource::Notes,
                timestamp,
                content: body,
                url: None,
                title: Some(title),
                tags: Vec::new(),
                attachment_segments: Vec::new(),
                metadata,
            })
        })
        .collect();

    let has_attachment_placeholders = cues.iter().any(|cue| {
        matches!(cue.source, CueSource::Notes) && attachment_placeholder_count(&cue.content) > 0
    });

    if has_attachment_placeholders {
        let media_notes = load_media_notes(since).unwrap_or_default();
        let web_preview_notes = load_web_preview_notes(since).unwrap_or_default();
        let map_notes = load_map_notes(since).unwrap_or_default();
        enrich_cues_with_attachments(&mut cues, &media_notes, &web_preview_notes, &map_notes);
    }

    Ok(cues)
}

fn normalize_plaintext(body: &str) -> (String, usize) {
    let attachment_placeholders = body
        .chars()
        .filter(|c| *c == ATTACHMENT_PLACEHOLDER)
        .count();
    if attachment_placeholders == 0 {
        return (body.to_string(), 0);
    }

    (
        body.replace(ATTACHMENT_PLACEHOLDER, ATTACHMENT_LABEL),
        attachment_placeholders,
    )
}

#[cfg(test)]
mod tests {
    use super::normalize_plaintext;
    use crate::notes::ATTACHMENT_LABEL;

    #[test]
    fn normalize_plaintext_replaces_attachment_placeholder_chars() {
        let body = format!("before{}after", '\u{fffc}');

        let (normalized, placeholders) = normalize_plaintext(&body);

        assert_eq!(normalized, format!("before{ATTACHMENT_LABEL}after"));
        assert_eq!(placeholders, 1);
    }

    #[test]
    fn normalize_plaintext_keeps_regular_text_unchanged() {
        let (normalized, placeholders) = normalize_plaintext("plain text note");

        assert_eq!(normalized, "plain text note");
        assert_eq!(placeholders, 0);
    }
}
