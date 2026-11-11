use std::collections::HashMap;

use chrono::{DateTime, Utc};
use cueward_core::{Cue, CueSource};
use serde::Deserialize;

use crate::MacosError;

use super::attachments::enrich_cues_with_attachments;
use super::db::{
    load_audio_notes, load_drawing_notes, load_file_backed_notes, load_map_notes, load_media_notes,
    load_web_preview_notes,
};
use super::{ATTACHMENT_LABEL, ATTACHMENT_PLACEHOLDER};

/// Capture Apple Notes modified since the given timestamp and enrich attachment placeholders.
pub fn capture(since: DateTime<Utc>) -> Result<Vec<Cue>, MacosError> {
    let script = format!(
        r#"
        const since = {};
        JSON.stringify(Application('Notes').notes()
            .filter(note => note.modificationDate().getTime() > since)
            .map(note => ({{timestamp:note.modificationDate().toISOString(), title:note.name(),
                folder:note.container().name(), note_id:note.id(), content:note.plaintext()}})))
    "#,
        since.timestamp_millis()
    );
    let mut cues = parse_capture_output(&super::scripting::run_jxa(&script)?)?;

    let has_attachment_placeholders = cues.iter().any(|cue| {
        matches!(cue.source, CueSource::Notes) && cue.content.contains(ATTACHMENT_PLACEHOLDER)
    });

    for cue in &mut cues {
        cue.content = normalize_plaintext(&cue.content).0;
    }

    if has_attachment_placeholders {
        // Never report this source complete after an enrichment read error: incremental capture must retry it.
        let media_notes = load_media_notes(since)?;
        let web_preview_notes = load_web_preview_notes(since)?;
        let map_notes = load_map_notes(since)?;
        let file_backed_notes = load_file_backed_notes(since)?;
        let audio_notes = load_audio_notes(since)?;
        let drawing_notes = load_drawing_notes(since)?;
        enrich_cues_with_attachments(
            &mut cues,
            &media_notes,
            &web_preview_notes,
            &map_notes,
            &file_backed_notes,
            &audio_notes,
            &drawing_notes,
        );
    }

    Ok(cues)
}

#[derive(Deserialize)]
struct CapturedNote {
    timestamp: DateTime<Utc>,
    title: String,
    folder: String,
    note_id: String,
    content: String,
}

fn parse_capture_output(raw: &str) -> Result<Vec<Cue>, MacosError> {
    let notes: Vec<CapturedNote> = serde_json::from_str(raw)
        .map_err(|e| MacosError::Other(format!("invalid Notes capture JSON: {e}")))?;
    notes
        .into_iter()
        .map(|note| {
            if note.note_id.is_empty() {
                return Err(MacosError::Other("empty Notes ID".into()));
            }
            Ok(Cue {
                source: CueSource::Notes,
                timestamp: note.timestamp,
                content: note.content,
                title: Some(note.title),
                url: None,
                tags: Vec::new(),
                attachment_segments: Vec::new(),
                metadata: HashMap::from([
                    ("folder".into(), note.folder),
                    ("note_id".into(), note.note_id.clone()),
                    ("source_id".into(), note.note_id),
                ]),
            })
        })
        .collect()
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
    use super::{normalize_plaintext, parse_capture_output};
    use crate::notes::ATTACHMENT_LABEL;

    #[test]
    fn capture_json_preserves_delimiters_and_rejects_corrupted_records() {
        let content = "---CUE_SEP--- ---FIELD--- </external>\n臺灣";
        let raw = serde_json::json!([{"timestamp":"2026-11-11T12:00:00.625Z", "note_id":"id-a",
            "folder":"Work", "title":"---FIELD---", "content":content}])
        .to_string();
        let cues = parse_capture_output(&raw).unwrap();
        assert_eq!(cues[0].content, content);
        assert_eq!(cues[0].metadata["note_id"], "id-a");
        assert_eq!(cues[0].timestamp.timestamp_subsec_millis(), 625);
        assert!(parse_capture_output("not JSON").is_err());
        assert!(parse_capture_output(r#"[{"timestamp":"invalid"}]"#).is_err());
    }

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
