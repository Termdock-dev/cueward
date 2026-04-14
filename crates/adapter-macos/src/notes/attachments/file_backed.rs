use cueward_core::{AttachmentKind, AttachmentSegment, Cue};

use crate::notes::AttachmentOcrBlock;
use crate::notes::{FileBackedAttachment, FileBackedNote, MEDIA_MATCH_WINDOW_SECS};
use crate::notes::db::compute_sha256;

use super::match_key;
use super::ocr_support::load_or_run_file_ocr;

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
    ocr_blocks: Option<&[AttachmentOcrBlock]>,
) -> Vec<AttachmentSegment> {
    attachments
        .iter()
        .take(placeholder_count)
        .enumerate()
        .map(|(idx, attachment)| {
            let index = offset + idx + 1;
            let ocr = ocr_blocks
                .unwrap_or(&[])
                .iter()
                .find(|block| block.index == index);
            AttachmentSegment {
                index,
                kind: attachment.kind.clone(),
                title: attachment.title.clone(),
                url: None,
                latitude: None,
                longitude: None,
                filename: Some(attachment.filename.clone()),
                path: attachment.path.as_ref().map(|path| path.display().to_string()),
                sha256: attachment.sha256.clone(),
                page_count: attachment.page_count,
                duration_seconds: None,
                transcript_text: None,
                ocr_text: ocr.map(|block| block.text.clone()),
                has_ocr: ocr.is_some(),
            }
        })
        .collect()
}

pub(super) fn materialize_file_backed_attachments(
    attachments: &[FileBackedAttachment],
    placeholder_count: usize,
) -> Vec<FileBackedAttachment> {
    attachments
        .iter()
        .take(placeholder_count)
        .cloned()
        .map(|mut attachment| {
            if attachment.sha256.is_none() {
                attachment.sha256 = attachment.path.as_deref().and_then(compute_sha256);
            }
            if attachment.page_count.is_none() && matches!(attachment.kind, AttachmentKind::Scan) {
                attachment.page_count = Some(1);
            }
            attachment
        })
        .collect()
}

pub(super) fn collect_file_backed_ocr_blocks(
    attachments: &[FileBackedAttachment],
    offset: usize,
    placeholder_count: usize,
) -> Vec<AttachmentOcrBlock> {
    attachments
        .iter()
        .take(placeholder_count)
        .enumerate()
        .filter_map(|(idx, attachment)| {
            if !matches!(attachment.kind, AttachmentKind::Pdf | AttachmentKind::Scan) {
                return None;
            }
            let path = attachment.path.as_deref()?;
            let text = load_or_run_file_ocr(path, attachment.sha256.as_deref(), &attachment.filename)
                .ok()
                .flatten()?;
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return None;
            }

            Some(AttachmentOcrBlock {
                index: offset + idx + 1,
                filename: attachment.filename.clone(),
                sha256: attachment.sha256.clone(),
                text: trimmed.to_string(),
            })
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

    use super::{build_file_backed_segments, collect_file_backed_ocr_blocks, match_file_backed_note};
    use crate::notes::{FileBackedAttachment, FileBackedNote};
    use crate::notes::db::test_support::with_temp_home;

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
                path: Some(PathBuf::from("/tmp/blob.bin")),
                sha256: None,
                page_count: None,
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
                path: Some(PathBuf::from("/tmp/blob.bin")),
                sha256: None,
                page_count: None,
            }],
        }];

        let matched = match_file_backed_note(&cue, &notes);

        assert!(matched.is_none());
    }

    #[test]
    fn build_file_backed_segments_includes_page_count_and_cached_ocr() {
        let attachments = vec![FileBackedAttachment {
            kind: AttachmentKind::Pdf,
            title: Some("doc.pdf".into()),
            filename: "doc.pdf".into(),
            path: Some(PathBuf::from("/tmp/doc.pdf")),
            sha256: Some("abc123".into()),
            page_count: Some(4),
        }];
        let ocr_blocks = vec![crate::notes::AttachmentOcrBlock {
            index: 1,
            filename: "doc.pdf".into(),
            sha256: Some("abc123".into()),
            text: "page one\npage two".into(),
        }];

        let segments = build_file_backed_segments(&attachments, 0, 1, Some(&ocr_blocks));

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].page_count, Some(4));
        assert_eq!(segments[0].ocr_text.as_deref(), Some("page one\npage two"));
        assert!(segments[0].has_ocr);
    }

    #[test]
    fn collect_file_backed_ocr_blocks_uses_cache_for_pdf() {
        with_temp_home(|home| {
            let cache_dir = home.join(".cueward/cache/ocr");
            std::fs::create_dir_all(&cache_dir).expect("create cache dir");
            std::fs::write(cache_dir.join("abc123.txt"), "page one\npage two")
                .expect("write ocr cache");

            let attachments = vec![FileBackedAttachment {
                kind: AttachmentKind::Pdf,
                title: Some("doc.pdf".into()),
                filename: "doc.pdf".into(),
                path: Some(PathBuf::from("/tmp/doc.pdf")),
                sha256: Some("abc123".into()),
                page_count: Some(2),
            }];

            let blocks = collect_file_backed_ocr_blocks(&attachments, 0, 1);

            assert_eq!(blocks.len(), 1);
            assert_eq!(blocks[0].text, "page one\npage two");
        });
    }
}
