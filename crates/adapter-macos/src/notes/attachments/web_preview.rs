use cueward_core::{AttachmentKind, AttachmentSegment, Cue};

use crate::notes::{WebPreviewAttachment, WebPreviewNote, MEDIA_MATCH_WINDOW_SECS};

use super::match_key;

pub(super) fn labels_for_web_previews(
    attachments: &[WebPreviewAttachment],
    placeholder_count: usize,
) -> Vec<String> {
    attachments
        .iter()
        .take(placeholder_count)
        .map(|attachment| {
            attachment
                .title
                .clone()
                .unwrap_or_else(|| attachment.url.clone())
        })
        .collect()
}

pub(super) fn build_web_preview_segments(
    attachments: &[WebPreviewAttachment],
    offset: usize,
    placeholder_count: usize,
) -> Vec<AttachmentSegment> {
    attachments
        .iter()
        .take(placeholder_count)
        .enumerate()
        .map(|(idx, attachment)| AttachmentSegment {
            index: offset + idx + 1,
            kind: AttachmentKind::WebPreview,
            title: attachment
                .title
                .clone()
                .or_else(|| Some(attachment.url.clone())),
            url: Some(attachment.url.clone()),
            latitude: None,
            longitude: None,
            filename: None,
            path: None,
            sha256: None,
            ocr_text: None,
            has_ocr: false,
        })
        .collect()
}

pub(super) fn match_web_preview_note<'a>(
    cue: &Cue,
    web_preview_notes: &'a [WebPreviewNote],
) -> Option<&'a WebPreviewNote> {
    let cue_ts = cue.timestamp.timestamp();
    let cue_title = cue.title.as_deref();

    web_preview_notes
        .iter()
        .filter(|note| (note.timestamp - cue_ts).abs() <= MEDIA_MATCH_WINDOW_SECS)
        .min_by_key(|note| match_key(cue_title, cue_ts, note.title.as_deref(), note.timestamp))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::{TimeZone, Utc};
    use cueward_core::{AttachmentKind, Cue, CueSource};

    use crate::notes::{WebPreviewAttachment, WebPreviewNote};

    #[test]
    fn enrich_cues_with_web_preview_emits_structured_segments() {
        let timestamp = Utc.with_ymd_and_hms(2026, 4, 10, 9, 0, 0).unwrap();
        let mut cues = vec![Cue {
            source: CueSource::Notes,
            timestamp,
            content: "[Attachment]".into(),
            url: None,
            title: Some("Working with Context".into()),
            tags: Vec::new(),
            attachment_segments: Vec::new(),
            metadata: HashMap::new(),
        }];
        let web_preview_notes = vec![WebPreviewNote {
            timestamp: timestamp.timestamp(),
            title: Some("Working with Context".into()),
            attachments: vec![WebPreviewAttachment {
                title: Some("Cursor Docs".into()),
                url: "https://docs.cursor.com/guides/working-with-context".into(),
            }],
        }];

        super::super::enrich_cues_with_attachments(&mut cues, &[], &web_preview_notes, &[]);

        assert_eq!(cues[0].content, "[Attachment 1: Cursor Docs]");
        assert_eq!(cues[0].attachment_segments.len(), 1);
        assert!(matches!(
            cues[0].attachment_segments[0].kind,
            AttachmentKind::WebPreview
        ));
        assert_eq!(
            cues[0].attachment_segments[0].title.as_deref(),
            Some("Cursor Docs")
        );
        assert_eq!(
            cues[0].attachment_segments[0].url.as_deref(),
            Some("https://docs.cursor.com/guides/working-with-context")
        );
        assert_eq!(cues[0].attachment_segments[0].path, None);
        assert_eq!(cues[0].attachment_segments[0].filename, None);
        assert!(!cues[0].attachment_segments[0].has_ocr);
    }

    #[test]
    fn enrich_cues_with_web_preview_falls_back_to_url_when_title_missing() {
        let timestamp = Utc.with_ymd_and_hms(2026, 4, 10, 9, 0, 0).unwrap();
        let mut cues = vec![Cue {
            source: CueSource::Notes,
            timestamp,
            content: "[Attachment]".into(),
            url: None,
            title: Some("Working with Context".into()),
            tags: Vec::new(),
            attachment_segments: Vec::new(),
            metadata: HashMap::new(),
        }];
        let web_preview_notes = vec![WebPreviewNote {
            timestamp: timestamp.timestamp(),
            title: Some("Working with Context".into()),
            attachments: vec![WebPreviewAttachment {
                title: None,
                url: "https://docs.cursor.com/guides/working-with-context".into(),
            }],
        }];

        super::super::enrich_cues_with_attachments(&mut cues, &[], &web_preview_notes, &[]);

        assert_eq!(
            cues[0].content,
            "[Attachment 1: https://docs.cursor.com/guides/working-with-context]"
        );
        assert_eq!(
            cues[0].attachment_segments[0].title.as_deref(),
            Some("https://docs.cursor.com/guides/working-with-context")
        );
    }
}
