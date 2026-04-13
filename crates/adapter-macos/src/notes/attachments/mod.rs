mod image;

use std::collections::HashMap;

use cueward_core::{AttachmentSegment, Cue, CueSource};

use super::{ATTACHMENT_LABEL, AttachmentOcrBlock, MEDIA_MATCH_WINDOW_SECS, MediaAttachment, MediaNote};
use image::{collect_attachment_ocr_blocks, materialize_attachments};

pub(crate) fn enrich_cues_with_attachments(cues: &mut [Cue], media_notes: &[MediaNote]) {
    for cue in cues.iter_mut() {
        if !matches!(cue.source, CueSource::Notes) {
            continue;
        }

        let placeholder_count = attachment_placeholder_count(&cue.content);
        if placeholder_count == 0 {
            continue;
        }

        let Some(media_note) = match_media_note(cue, media_notes) else {
            continue;
        };

        if media_note.attachments.is_empty() {
            continue;
        }

        let attachments = materialize_attachments(&media_note.attachments, placeholder_count);
        if attachments.is_empty() {
            continue;
        }

        cue.content = replace_attachment_labels(&cue.content, &attachments, placeholder_count);

        let ocr_blocks = collect_attachment_ocr_blocks(&attachments, placeholder_count);
        if !ocr_blocks.is_empty() {
            cue.content = append_attachment_ocr(&cue.content, &ocr_blocks);
        }

        cue.attachment_segments =
            build_attachment_segments(&attachments, placeholder_count, Some(&ocr_blocks));
    }
}

#[cfg(test)]
pub(crate) fn enrich_cues_with_media(cues: &mut [Cue], media_notes: &[MediaNote]) {
    for cue in cues.iter_mut() {
        if !matches!(cue.source, CueSource::Notes) {
            continue;
        }

        let placeholder_count = attachment_placeholder_count(&cue.content);
        if placeholder_count == 0 {
            continue;
        }

        let Some(media_note) = match_media_note(cue, media_notes) else {
            continue;
        };

        if media_note.attachments.is_empty() {
            continue;
        }

        cue.content =
            replace_attachment_labels(&cue.content, &media_note.attachments, placeholder_count);

        cue.attachment_segments =
            build_attachment_segments(&media_note.attachments, placeholder_count, None);
    }
}

pub(crate) fn attachment_placeholder_count(content: &str) -> usize {
    content.matches(ATTACHMENT_LABEL).count()
}

fn match_media_note<'a>(cue: &Cue, media_notes: &'a [MediaNote]) -> Option<&'a MediaNote> {
    let cue_ts = cue.timestamp.timestamp();
    let cue_title = cue.title.as_deref();

    media_notes
        .iter()
        .filter(|note| (note.timestamp - cue_ts).abs() <= MEDIA_MATCH_WINDOW_SECS)
        .min_by_key(|note| {
            let title_penalty = match (cue_title, note.title.as_deref()) {
                (Some(cue_title), Some(note_title)) if cue_title == note_title => 0,
                (Some(_), Some(_)) => 1,
                _ => 2,
            };
            (title_penalty, (note.timestamp - cue_ts).abs())
        })
}

fn replace_attachment_labels(
    body: &str,
    attachments: &[MediaAttachment],
    placeholder_count: usize,
) -> String {
    if placeholder_count == 0 {
        return body.to_string();
    }

    let mut result = body.to_string();
    for (idx, attachment) in attachments.iter().enumerate() {
        if idx >= placeholder_count {
            break;
        }

        let replacement = format!("[Attachment {}: {}]", idx + 1, attachment.filename);
        result = result.replacen(ATTACHMENT_LABEL, &replacement, 1);
    }

    result
}

fn append_attachment_ocr(content: &str, blocks: &[AttachmentOcrBlock]) -> String {
    if blocks.is_empty() {
        return content.to_string();
    }

    let mut sections = Vec::with_capacity(blocks.len());
    for block in blocks {
        sections.push(format!(
            "[Attachment {} OCR: {}]\n{}",
            block.index, block.filename, block.text
        ));
    }

    format!("{content}\n\n{}", sections.join("\n\n"))
}

fn build_attachment_segments(
    attachments: &[MediaAttachment],
    placeholder_count: usize,
    ocr_blocks: Option<&[AttachmentOcrBlock]>,
) -> Vec<AttachmentSegment> {
    let ocr_by_attachment: HashMap<String, &AttachmentOcrBlock> = ocr_blocks
        .unwrap_or(&[])
        .iter()
        .map(|block| {
            (
                attachment_lookup_key(block.sha256.as_deref(), &block.filename),
                block,
            )
        })
        .collect();

    attachments
        .iter()
        .take(placeholder_count)
        .enumerate()
        .map(|(idx, attachment)| {
            let key = attachment_lookup_key(attachment.sha256.as_deref(), &attachment.filename);
            let ocr = ocr_by_attachment.get(&key);
            AttachmentSegment {
                index: idx + 1,
                filename: attachment.filename.clone(),
                path: attachment.path.display().to_string(),
                sha256: attachment.sha256.clone(),
                ocr_text: ocr.map(|block| block.text.clone()),
                has_ocr: ocr.is_some(),
            }
        })
        .collect()
}

fn attachment_lookup_key(sha256: Option<&str>, filename: &str) -> String {
    sha256.unwrap_or(filename).to_string()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use chrono::{TimeZone, Utc};
    use cueward_core::{Cue, CueSource};

    use super::{
        AttachmentOcrBlock, MediaAttachment, MediaNote, append_attachment_ocr,
        attachment_placeholder_count, build_attachment_segments, enrich_cues_with_media, replace_attachment_labels,
    };

    #[test]
    fn replace_attachment_labels_uses_filenames() {
        let body = "[Attachment]\n[Attachment]";
        let attachments = vec![
            MediaAttachment {
                filename: "one.png".into(),
                path: PathBuf::from("/tmp/one.png"),
                sha256: None,
            },
            MediaAttachment {
                filename: "two.jpg".into(),
                path: PathBuf::from("/tmp/two.jpg"),
                sha256: None,
            },
        ];

        let replaced = replace_attachment_labels(body, &attachments, 2);

        assert_eq!(replaced, "[Attachment 1: one.png]\n[Attachment 2: two.jpg]");
    }

    #[test]
    fn enrich_cues_with_media_adds_paths_and_filenames() {
        let timestamp = Utc.with_ymd_and_hms(2026, 4, 9, 23, 42, 54).unwrap();
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
        let media_notes = vec![MediaNote {
            timestamp: timestamp.timestamp() - 1,
            title: None,
            attachments: vec![MediaAttachment {
                filename: "scan.jpg".into(),
                path: PathBuf::from("/tmp/scan.jpg"),
                sha256: Some("abc123".into()),
            }],
        }];

        enrich_cues_with_media(&mut cues, &media_notes);

        assert_eq!(cues[0].content, "[Attachment 1: scan.jpg]");
        assert_eq!(cues[0].attachment_segments.len(), 1);
        assert_eq!(cues[0].attachment_segments[0].filename, "scan.jpg");
        assert_eq!(cues[0].attachment_segments[0].path, "/tmp/scan.jpg");
        assert_eq!(
            cues[0].attachment_segments[0].sha256.as_deref(),
            Some("abc123")
        );
        assert!(!cues[0].attachment_segments[0].has_ocr);
    }

    #[test]
    fn append_attachment_ocr_adds_readable_sections() {
        let content = "[Attachment 1: image.png]";
        let blocks = vec![AttachmentOcrBlock {
            index: 1,
            filename: "image.png".into(),
            sha256: Some("hash1".into()),
            text: "detected text".into(),
        }];

        let combined = append_attachment_ocr(content, &blocks);

        assert_eq!(
            combined,
            "[Attachment 1: image.png]\n\n[Attachment 1 OCR: image.png]\ndetected text"
        );
    }

    #[test]
    fn attachment_placeholder_count_counts_remaining_plain_labels() {
        assert_eq!(
            attachment_placeholder_count("[Attachment]\n[Attachment]"),
            2
        );
        assert_eq!(attachment_placeholder_count("[Attachment 1: image.png]"), 0);
    }

    #[test]
    fn build_attachment_segments_includes_ocr_fields() {
        let attachments = vec![MediaAttachment {
            filename: "image.png".into(),
            path: PathBuf::from("/tmp/image.png"),
            sha256: Some("hash1".into()),
        }];
        let blocks = vec![AttachmentOcrBlock {
            index: 1,
            filename: "image.png".into(),
            sha256: Some("hash1".into()),
            text: "detected text".into(),
        }];

        let segments = build_attachment_segments(&attachments, 1, Some(&blocks));

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].index, 1);
        assert_eq!(segments[0].filename, "image.png");
        assert_eq!(segments[0].path, "/tmp/image.png");
        assert_eq!(segments[0].sha256.as_deref(), Some("hash1"));
        assert_eq!(segments[0].ocr_text.as_deref(), Some("detected text"));
        assert!(segments[0].has_ocr);
    }

    #[test]
    fn build_attachment_segments_prefers_sha256_over_filename() {
        let attachments = vec![
            MediaAttachment {
                filename: "image.png".into(),
                path: PathBuf::from("/tmp/one.png"),
                sha256: Some("hash-a".into()),
            },
            MediaAttachment {
                filename: "image.png".into(),
                path: PathBuf::from("/tmp/two.png"),
                sha256: Some("hash-b".into()),
            },
        ];
        let blocks = vec![
            AttachmentOcrBlock {
                index: 1,
                filename: "image.png".into(),
                sha256: Some("hash-a".into()),
                text: "first".into(),
            },
            AttachmentOcrBlock {
                index: 2,
                filename: "image.png".into(),
                sha256: Some("hash-b".into()),
                text: "second".into(),
            },
        ];

        let segments = build_attachment_segments(&attachments, 2, Some(&blocks));

        assert_eq!(segments[0].ocr_text.as_deref(), Some("first"));
        assert_eq!(segments[1].ocr_text.as_deref(), Some("second"));
    }

    #[test]
    fn append_attachment_ocr_keeps_original_attachment_indices() {
        let content = "[Attachment 1: one.png]\n[Attachment 2: two.png]";
        let blocks = vec![AttachmentOcrBlock {
            index: 2,
            filename: "two.png".into(),
            sha256: Some("hash2".into()),
            text: "detected text".into(),
        }];

        let combined = append_attachment_ocr(content, &blocks);

        assert!(combined.contains("[Attachment 2 OCR: two.png]"));
    }
}
