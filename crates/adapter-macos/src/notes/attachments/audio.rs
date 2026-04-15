use cueward_core::{AttachmentKind, AttachmentSegment, Cue};

use crate::notes::{AudioAttachment, AudioNote, MEDIA_MATCH_WINDOW_SECS};

use super::match_key;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AudioTranscriptBlock {
    pub(super) index: usize,
    pub(super) filename: String,
    pub(super) text: String,
}

pub(super) fn labels_for_audio(attachments: &[AudioAttachment], placeholder_count: usize) -> Vec<String> {
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

pub(super) fn materialize_audio_attachments(
    attachments: &[AudioAttachment],
    placeholder_count: usize,
) -> Vec<AudioAttachment> {
    attachments
        .iter()
        .take(placeholder_count)
        .cloned()
        .collect()
}

pub(super) fn collect_audio_transcript_blocks(
    attachments: &[AudioAttachment],
    offset: usize,
    placeholder_count: usize,
) -> Vec<AudioTranscriptBlock> {
    attachments
        .iter()
        .take(placeholder_count)
        .enumerate()
        .filter_map(|(idx, attachment)| {
            let text = attachment.transcript_text.as_deref()?.trim();
            if text.is_empty() {
                return None;
            }

            Some(AudioTranscriptBlock {
                index: offset + idx + 1,
                filename: attachment.filename.clone(),
                text: text.to_string(),
            })
        })
        .collect()
}

pub(super) fn append_audio_transcripts(content: &str, blocks: &[AudioTranscriptBlock]) -> String {
    if blocks.is_empty() {
        return content.to_string();
    }

    let mut sections = Vec::with_capacity(blocks.len());
    for block in blocks {
        sections.push(format!(
            "[Attachment {} Transcript: {}]\n{}",
            block.index, block.filename, block.text
        ));
    }

    format!("{content}\n\n{}", sections.join("\n\n"))
}

pub(super) fn build_audio_segments(
    attachments: &[AudioAttachment],
    offset: usize,
    placeholder_count: usize,
) -> Vec<AttachmentSegment> {
    attachments
        .iter()
        .take(placeholder_count)
        .enumerate()
        .map(|(idx, attachment)| AttachmentSegment {
            index: offset + idx + 1,
            kind: AttachmentKind::Audio,
            title: attachment.title.clone(),
            url: None,
            latitude: None,
            longitude: None,
            filename: Some(attachment.filename.clone()),
            path: Some(attachment.path.display().to_string()),
            sha256: attachment.sha256.clone(),
            page_count: None,
            duration_seconds: attachment.duration_seconds,
            transcript_text: attachment.transcript_text.clone(),
            ocr_text: None,
            has_ocr: false,
        })
        .collect()
}

pub(super) fn match_audio_note<'a>(cue: &Cue, notes: &'a [AudioNote]) -> Option<&'a AudioNote> {
    let cue_ts = cue.timestamp.timestamp();
    let cue_title = cue.title.as_deref();

    notes
        .iter()
        .filter(|note| (note.timestamp - cue_ts).abs() <= MEDIA_MATCH_WINDOW_SECS)
        .min_by_key(|note| match_key(cue_title, cue_ts, note.title.as_deref(), note.timestamp))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use chrono::{TimeZone, Utc};
    use cueward_core::{AttachmentKind, Cue, CueSource};

    use crate::notes::{AudioAttachment, AudioNote, MediaAttachment, MediaNote};

    #[test]
    fn enrich_cues_with_audio_emits_audio_segment_and_transcript() {
        let timestamp = Utc.with_ymd_and_hms(2026, 4, 14, 5, 29, 52).unwrap();
        let mut cues = vec![Cue {
            source: CueSource::Notes,
            timestamp,
            content: "[Attachment]".into(),
            url: None,
            title: Some("voai-test.wav".into()),
            tags: Vec::new(),
            attachment_segments: Vec::new(),
            metadata: HashMap::new(),
        }];
        let audio_notes = vec![AudioNote {
            timestamp: timestamp.timestamp(),
            title: Some("voai-test.wav".into()),
            attachments: vec![AudioAttachment {
                title: Some("voai-test.wav".into()),
                filename: "voai-test.wav".into(),
                path: PathBuf::from("/tmp/voai-test.wav"),
                sha256: Some("audio123".into()),
                duration_seconds: Some(3.769),
                transcript_text: Some("老闆你好我是龍工這是語音測試。".into()),
            }],
        }];

        super::super::enrich_cues_with_attachments(&mut cues, &[], &[], &[], &[], &audio_notes, &[]);

        assert_eq!(
            cues[0].content,
            "[Attachment 1: voai-test.wav]\n\n[Attachment 1 Transcript: voai-test.wav]\n老闆你好我是龍工這是語音測試。"
        );
        assert_eq!(cues[0].attachment_segments.len(), 1);
        assert!(matches!(
            cues[0].attachment_segments[0].kind,
            AttachmentKind::Audio
        ));
        assert_eq!(
            cues[0].attachment_segments[0].path.as_deref(),
            Some("/tmp/voai-test.wav")
        );
        assert_eq!(
            cues[0].attachment_segments[0].sha256.as_deref(),
            Some("audio123")
        );
        assert_eq!(cues[0].attachment_segments[0].duration_seconds, Some(3.769));
        assert_eq!(
            cues[0].attachment_segments[0].transcript_text.as_deref(),
            Some("老闆你好我是龍工這是語音測試。")
        );
        assert!(!cues[0].attachment_segments[0].has_ocr);
    }

    #[test]
    fn enrich_cues_with_media_and_audio_keeps_transcript_index_in_sync() {
        let timestamp = Utc.with_ymd_and_hms(2026, 4, 14, 5, 29, 52).unwrap();
        let mut cues = vec![Cue {
            source: CueSource::Notes,
            timestamp,
            content: "[Attachment]\n[Attachment]".into(),
            url: None,
            title: Some("voai-test.wav".into()),
            tags: Vec::new(),
            attachment_segments: Vec::new(),
            metadata: HashMap::new(),
        }];
        let media_notes = vec![MediaNote {
            timestamp: timestamp.timestamp(),
            title: Some("voai-test.wav".into()),
            attachments: vec![MediaAttachment {
                filename: "scan.jpg".into(),
                path: PathBuf::from("/tmp/scan.jpg"),
                sha256: Some("img123".into()),
            }],
        }];
        let audio_notes = vec![AudioNote {
            timestamp: timestamp.timestamp(),
            title: Some("voai-test.wav".into()),
            attachments: vec![AudioAttachment {
                title: Some("voai-test.wav".into()),
                filename: "voai-test.wav".into(),
                path: PathBuf::from("/tmp/voai-test.wav"),
                sha256: Some("audio123".into()),
                duration_seconds: Some(3.769),
                transcript_text: Some("老闆你好我是龍工這是語音測試。".into()),
            }],
        }];

        super::super::enrich_cues_with_attachments(
            &mut cues,
            &media_notes,
            &[],
            &[],
            &[],
            &audio_notes,
            &[],
        );

        assert_eq!(
            cues[0].content,
            "[Attachment 1: scan.jpg]\n[Attachment 2: voai-test.wav]\n\n[Attachment 2 Transcript: voai-test.wav]\n老闆你好我是龍工這是語音測試。"
        );
        assert_eq!(cues[0].attachment_segments.len(), 2);
        assert!(matches!(
            cues[0].attachment_segments[1].kind,
            AttachmentKind::Audio
        ));
        assert_eq!(cues[0].attachment_segments[1].index, 2);
    }
}
