use cueward_core::{AttachmentKind, AttachmentSegment, Cue};

use crate::notes::{MEDIA_MATCH_WINDOW_SECS, MapAttachment, MapNote};

use super::match_key;

pub(super) fn labels_for_maps(attachments: &[MapAttachment], placeholder_count: usize) -> Vec<String> {
    attachments
        .iter()
        .take(placeholder_count)
        .map(|attachment| {
            attachment
                .title
                .clone()
                .or_else(|| attachment.url.clone())
                .unwrap_or_else(|| "Map".to_string())
        })
        .collect()
}

pub(super) fn build_map_segments(
    attachments: &[MapAttachment],
    offset: usize,
    placeholder_count: usize,
) -> Vec<AttachmentSegment> {
    attachments
        .iter()
        .take(placeholder_count)
        .enumerate()
        .map(|(idx, attachment)| AttachmentSegment {
            index: offset + idx + 1,
            kind: AttachmentKind::Map,
            title: attachment.title.clone(),
            url: attachment.url.clone(),
            latitude: Some(attachment.latitude),
            longitude: Some(attachment.longitude),
            filename: None,
            path: None,
            sha256: None,
            duration_seconds: None,
            transcript_text: None,
            ocr_text: None,
            has_ocr: false,
        })
        .collect()
}

pub(super) fn match_map_note<'a>(cue: &Cue, map_notes: &'a [MapNote]) -> Option<&'a MapNote> {
    let cue_ts = cue.timestamp.timestamp();
    let cue_title = cue.title.as_deref();

    map_notes
        .iter()
        .filter(|note| (note.timestamp - cue_ts).abs() <= MEDIA_MATCH_WINDOW_SECS)
        .min_by_key(|note| match_key(cue_title, cue_ts, note.title.as_deref(), note.timestamp))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::{TimeZone, Utc};
    use cueward_core::{AttachmentKind, Cue, CueSource};

    use crate::notes::{MapAttachment, MapNote};

    #[test]
    fn enrich_cues_with_map_emits_structured_map_segment() {
        let timestamp = Utc.with_ymd_and_hms(2026, 4, 13, 16, 24, 54).unwrap();
        let mut cues = vec![Cue {
            source: CueSource::Notes,
            timestamp,
            content: "[Attachment]".into(),
            url: None,
            title: Some("新增備忘錄".into()),
            tags: Vec::new(),
            attachment_segments: Vec::new(),
            metadata: HashMap::new(),
        }];
        let map_notes = vec![MapNote {
            timestamp: timestamp.timestamp(),
            title: Some("新增備忘錄".into()),
            attachments: vec![MapAttachment {
                title: Some("屏東縣立棒球場".into()),
                url: Some("https://maps.apple.com/place?...".into()),
                latitude: 22.657349,
                longitude: 120.485956,
            }],
        }];

        super::super::enrich_cues_with_attachments(&mut cues, &[], &[], &map_notes, &[], &[]);

        assert_eq!(cues[0].content, "[Attachment 1: 屏東縣立棒球場]");
        assert_eq!(cues[0].attachment_segments.len(), 1);
        assert!(matches!(
            cues[0].attachment_segments[0].kind,
            AttachmentKind::Map
        ));
        assert_eq!(
            cues[0].attachment_segments[0].title.as_deref(),
            Some("屏東縣立棒球場")
        );
        assert_eq!(
            cues[0].attachment_segments[0].url.as_deref(),
            Some("https://maps.apple.com/place?...")
        );
        assert_eq!(cues[0].attachment_segments[0].latitude, Some(22.657349));
        assert_eq!(cues[0].attachment_segments[0].longitude, Some(120.485956));
        assert!(!cues[0].attachment_segments[0].has_ocr);
    }
}
