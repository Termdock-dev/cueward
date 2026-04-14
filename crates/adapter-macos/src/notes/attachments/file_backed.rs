use cueward_core::{AttachmentSegment, Cue};

use crate::notes::{FileBackedAttachment, FileBackedNote, MEDIA_MATCH_WINDOW_SECS};

use super::match_key;

const FILE_BACKED_TITLE_MATCH_WINDOW_SECS: i64 = 90;

pub(super) fn labels_for_file_backed(attachments: &[FileBackedAttachment], placeholder_count: usize) -> Vec<String> {
    attachments
        .iter()
        .take(placeholder_count)
        .map(|attachment| {
            attachment
                .title
                .clone()
                .unwrap_or_else(|| attachment.filename.clone())
        })
        .collect()
}

pub(super) fn build_file_backed_segments(
    attachments: &[FileBackedAttachment],
    offset: usize,
    placeholder_count: usize,
) -> Vec<AttachmentSegment> {
    attachments
        .iter()
        .take(placeholder_count)
        .enumerate()
        .map(|(idx, attachment)| AttachmentSegment {
            index: offset + idx + 1,
            kind: attachment.kind.clone(),
            title: attachment.title.clone(),
            url: None,
            latitude: None,
            longitude: None,
            filename: Some(attachment.filename.clone()),
            path: Some(attachment.path.display().to_string()),
            sha256: attachment.sha256.clone(),
            ocr_text: None,
            has_ocr: false,
        })
        .collect()
}

pub(super) fn match_file_backed_note<'a>(
    cue: &Cue,
    notes: &'a [FileBackedNote],
) -> Option<&'a FileBackedNote> {
    let cue_ts = cue.timestamp.timestamp();
    let cue_title = cue.title.as_deref();

    let strict_match = notes
        .iter()
        .filter(|note| (note.timestamp - cue_ts).abs() <= MEDIA_MATCH_WINDOW_SECS)
        .min_by_key(|note| match_key(cue_title, cue_ts, note.title.as_deref(), note.timestamp));

    if strict_match.is_some() {
        return strict_match;
    }

    let cue_title = cue_title?;
    notes
        .iter()
        .filter(|note| note.title.as_deref() == Some(cue_title))
        .filter(|note| (note.timestamp - cue_ts).abs() <= FILE_BACKED_TITLE_MATCH_WINDOW_SECS)
        .min_by_key(|note| (note.timestamp - cue_ts).abs())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use chrono::{TimeZone, Utc};
    use cueward_core::{AttachmentKind, Cue, CueSource};

    use super::match_file_backed_note;
    use crate::notes::{FileBackedAttachment, FileBackedNote};

    #[test]
    fn match_file_backed_note_handles_exact_title_with_timestamp_drift() {
        let cue = Cue {
            source: CueSource::Notes,
            timestamp: Utc.with_ymd_and_hms(2026, 4, 9, 23, 44, 17).unwrap(),
            content: "[Attachment]".into(),
            url: None,
            title: Some("新增備忘錄".into()),
            tags: Vec::new(),
            attachment_segments: Vec::new(),
            metadata: HashMap::new(),
        };
        let notes = vec![FileBackedNote {
            timestamp: Utc.with_ymd_and_hms(2026, 4, 9, 23, 42, 53)
                .unwrap()
                .timestamp(),
            title: Some("新增備忘錄".into()),
            attachments: vec![FileBackedAttachment {
                kind: AttachmentKind::Binary,
                title: None,
                filename: "blob.bin".into(),
                path: PathBuf::from("/tmp/blob.bin"),
                sha256: None,
            }],
        }];

        let matched = match_file_backed_note(&cue, &notes);

        assert!(matched.is_some());
    }

    #[test]
    fn match_file_backed_note_rejects_same_title_outside_fallback_window() {
        let cue = Cue {
            source: CueSource::Notes,
            timestamp: Utc.with_ymd_and_hms(2026, 4, 9, 23, 44, 17).unwrap(),
            content: "[Attachment]".into(),
            url: None,
            title: Some("新增備忘錄".into()),
            tags: Vec::new(),
            attachment_segments: Vec::new(),
            metadata: HashMap::new(),
        };
        let notes = vec![FileBackedNote {
            timestamp: Utc.with_ymd_and_hms(2026, 4, 9, 23, 42, 37)
                .unwrap()
                .timestamp(),
            title: Some("新增備忘錄".into()),
            attachments: vec![FileBackedAttachment {
                kind: AttachmentKind::Binary,
                title: None,
                filename: "blob.bin".into(),
                path: PathBuf::from("/tmp/blob.bin"),
                sha256: None,
            }],
        }];

        let matched = match_file_backed_note(&cue, &notes);

        assert!(matched.is_none());
    }
}
