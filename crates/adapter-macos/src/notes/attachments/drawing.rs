use cueward_core::{AttachmentKind, AttachmentSegment, Cue};

use crate::notes::{DrawingAttachment, DrawingNote, MEDIA_MATCH_WINDOW_SECS};

use super::match_key;

pub(super) fn labels_for_drawings(
    attachments: &[DrawingAttachment],
    placeholder_count: usize,
) -> Vec<String> {
    attachments
        .iter()
        .take(placeholder_count)
        .map(|attachment| {
            attachment
                .title
                .clone()
                .unwrap_or_else(|| "Drawing".to_string())
        })
        .collect()
}

pub(super) fn build_drawing_segments(
    attachments: &[DrawingAttachment],
    offset: usize,
    placeholder_count: usize,
) -> Vec<AttachmentSegment> {
    attachments
        .iter()
        .take(placeholder_count)
        .enumerate()
        .map(|(idx, attachment)| AttachmentSegment {
            index: offset + idx + 1,
            kind: AttachmentKind::Drawing,
            title: attachment.title.clone(),
            url: None,
            latitude: None,
            longitude: None,
            filename: None,
            path: None,
            sha256: None,
            page_count: None,
            duration_seconds: None,
            transcript_text: None,
            ocr_text: None,
            has_ocr: false,
        })
        .collect()
}

pub(super) fn match_drawing_note<'a>(
    cue: &Cue,
    notes: &'a [DrawingNote],
) -> Option<&'a DrawingNote> {
    let cue_ts = cue.timestamp.timestamp();
    let cue_title = cue.title.as_deref();

    notes
        .iter()
        .filter(|note| (note.timestamp - cue_ts).abs() <= MEDIA_MATCH_WINDOW_SECS)
        .min_by_key(|note| match_key(cue_title, cue_ts, note.title.as_deref(), note.timestamp))
}
