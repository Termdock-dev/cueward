use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{TimeZone, Utc};
use cueward_core::{AttachmentKind, Cue, CueSource};

use super::{
    AttachmentOcrBlock, MediaAttachment, MediaNote, append_attachment_ocr,
    attachment_placeholder_count, build_image_segments, enrich_cues_with_media,
    replace_attachment_labels,
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

    let replaced = replace_attachment_labels(
        body,
        &attachments
            .iter()
            .map(|attachment| attachment.filename.clone())
            .collect::<Vec<_>>(),
        2,
    );

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
    assert!(matches!(
        cues[0].attachment_segments[0].kind,
        AttachmentKind::Image
    ));
    assert_eq!(
        cues[0].attachment_segments[0].filename.as_deref(),
        Some("scan.jpg")
    );
    assert_eq!(
        cues[0].attachment_segments[0].path.as_deref(),
        Some("/tmp/scan.jpg")
    );
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
    assert_eq!(attachment_placeholder_count("[Attachment]\n[Attachment]"), 2);
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

    let segments = build_image_segments(&attachments, Some(&blocks));

    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].index, 1);
    assert!(matches!(segments[0].kind, AttachmentKind::Image));
    assert_eq!(segments[0].filename.as_deref(), Some("image.png"));
    assert_eq!(segments[0].path.as_deref(), Some("/tmp/image.png"));
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

    let segments = build_image_segments(&attachments, Some(&blocks));

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

#[test]
fn enrich_cues_with_attachments_emits_unresolved_when_no_media_matches() {
    let timestamp = Utc.with_ymd_and_hms(2026, 4, 10, 8, 0, 0).unwrap();
    let mut cues = vec![Cue {
        source: CueSource::Notes,
        timestamp,
        content: "[Attachment]".into(),
        url: None,
        title: Some("Unresolved".into()),
        tags: Vec::new(),
        attachment_segments: Vec::new(),
        metadata: HashMap::new(),
    }];

    super::enrich_cues_with_attachments(&mut cues, &[], &[], &[], &[]);

    assert_eq!(cues[0].attachment_segments.len(), 1);
    assert!(matches!(
        cues[0].attachment_segments[0].kind,
        AttachmentKind::Unresolved
    ));
    assert_eq!(cues[0].attachment_segments[0].filename, None);
    assert_eq!(cues[0].attachment_segments[0].path, None);
    assert!(!cues[0].attachment_segments[0].has_ocr);
}

#[test]
fn enrich_cues_with_attachments_emits_unresolved_when_match_has_no_attachments() {
    let timestamp = Utc.with_ymd_and_hms(2026, 4, 10, 8, 0, 0).unwrap();
    let mut cues = vec![Cue {
        source: CueSource::Notes,
        timestamp,
        content: "[Attachment]\n[Attachment]".into(),
        url: None,
        title: Some("Still unresolved".into()),
        tags: Vec::new(),
        attachment_segments: Vec::new(),
        metadata: HashMap::new(),
    }];
    let media_notes = vec![MediaNote {
        timestamp: timestamp.timestamp(),
        title: Some("Still unresolved".into()),
        attachments: Vec::new(),
    }];

    super::enrich_cues_with_attachments(&mut cues, &media_notes, &[], &[], &[]);

    assert_eq!(cues[0].attachment_segments.len(), 2);
    assert!(cues[0]
        .attachment_segments
        .iter()
        .all(|segment| matches!(segment.kind, AttachmentKind::Unresolved)));
    assert!(cues[0]
        .attachment_segments
        .iter()
        .all(|segment| segment.path.is_none() && segment.filename.is_none()));
}

#[test]
fn enrich_cues_with_attachments_combines_media_and_web_preview_segments() {
    let timestamp = Utc.with_ymd_and_hms(2026, 4, 10, 9, 0, 0).unwrap();
    let mut cues = vec![Cue {
        source: CueSource::Notes,
        timestamp,
        content: "[Attachment]\n[Attachment]".into(),
        url: None,
        title: Some("Mixed note".into()),
        tags: Vec::new(),
        attachment_segments: Vec::new(),
        metadata: HashMap::new(),
    }];
    let media_notes = vec![MediaNote {
        timestamp: timestamp.timestamp(),
        title: Some("Mixed note".into()),
        attachments: vec![MediaAttachment {
            filename: "scan.jpg".into(),
            path: PathBuf::from("/tmp/scan.jpg"),
            sha256: Some("abc123".into()),
        }],
    }];
    let web_preview_notes = vec![crate::notes::WebPreviewNote {
        timestamp: timestamp.timestamp(),
        title: Some("Mixed note".into()),
        attachments: vec![crate::notes::WebPreviewAttachment {
            title: Some("Cursor Docs".into()),
            url: "https://docs.cursor.com/guides/working-with-context".into(),
        }],
    }];

    super::enrich_cues_with_attachments(&mut cues, &media_notes, &web_preview_notes, &[], &[]);

    assert_eq!(
        cues[0].content,
        "[Attachment 1: scan.jpg]\n[Attachment 2: Cursor Docs]"
    );
    assert_eq!(cues[0].attachment_segments.len(), 2);
    assert_eq!(cues[0].attachment_segments[0].index, 1);
    assert_eq!(cues[0].attachment_segments[1].index, 2);
    assert!(matches!(
        cues[0].attachment_segments[0].kind,
        AttachmentKind::Image
    ));
    assert!(matches!(
        cues[0].attachment_segments[1].kind,
        AttachmentKind::WebPreview
    ));
}

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
    let map_notes = vec![crate::notes::MapNote {
        timestamp: timestamp.timestamp(),
        title: Some("新增備忘錄".into()),
        attachments: vec![crate::notes::MapAttachment {
            title: Some("屏東縣立棒球場".into()),
            url: Some("https://maps.apple.com/place?...".into()),
            latitude: 22.657349,
            longitude: 120.485956,
        }],
    }];

    super::enrich_cues_with_attachments(&mut cues, &[], &[], &map_notes, &[]);

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

#[test]
fn enrich_cues_with_attachments_combines_media_and_map_segments() {
    let timestamp = Utc.with_ymd_and_hms(2026, 4, 13, 16, 24, 54).unwrap();
    let mut cues = vec![Cue {
        source: CueSource::Notes,
        timestamp,
        content: "[Attachment]\n[Attachment]".into(),
        url: None,
        title: Some("新增備忘錄".into()),
        tags: Vec::new(),
        attachment_segments: Vec::new(),
        metadata: HashMap::new(),
    }];
    let media_notes = vec![MediaNote {
        timestamp: timestamp.timestamp(),
        title: Some("新增備忘錄".into()),
        attachments: vec![MediaAttachment {
            filename: "scan.jpg".into(),
            path: PathBuf::from("/tmp/scan.jpg"),
            sha256: Some("abc123".into()),
        }],
    }];
    let map_notes = vec![crate::notes::MapNote {
        timestamp: timestamp.timestamp(),
        title: Some("新增備忘錄".into()),
        attachments: vec![crate::notes::MapAttachment {
            title: Some("屏東縣立棒球場".into()),
            url: Some("https://maps.apple.com/place?...".into()),
            latitude: 22.657349,
            longitude: 120.485956,
        }],
    }];

    super::enrich_cues_with_attachments(&mut cues, &media_notes, &[], &map_notes, &[]);

    assert_eq!(
        cues[0].content,
        "[Attachment 1: scan.jpg]\n[Attachment 2: 屏東縣立棒球場]"
    );
    assert_eq!(cues[0].attachment_segments.len(), 2);
    assert_eq!(cues[0].attachment_segments[0].index, 1);
    assert_eq!(cues[0].attachment_segments[1].index, 2);
    assert!(matches!(
        cues[0].attachment_segments[0].kind,
        AttachmentKind::Image
    ));
    assert!(matches!(
        cues[0].attachment_segments[1].kind,
        AttachmentKind::Map
    ));
}

#[test]
fn enrich_cues_with_pdf_emits_file_backed_pdf_segment() {
    let timestamp = Utc.with_ymd_and_hms(2026, 4, 14, 0, 10, 15).unwrap();
    let mut cues = vec![Cue {
        source: CueSource::Notes,
        timestamp,
        content: "[Attachment]".into(),
        url: None,
        title: Some("test".into()),
        tags: Vec::new(),
        attachment_segments: Vec::new(),
        metadata: HashMap::new(),
    }];
    let file_backed_notes = vec![crate::notes::FileBackedNote {
        timestamp: timestamp.timestamp(),
        title: Some("test".into()),
        attachments: vec![crate::notes::FileBackedAttachment {
            kind: AttachmentKind::Pdf,
            title: Some("SK-INFLUX [V MB]_DS_C0919.pdf".into()),
            filename: "SK-INFLUX [V MB]_DS_C0919.pdf".into(),
            path: PathBuf::from("/tmp/SK-INFLUX [V MB]_DS_C0919.pdf"),
            sha256: Some("abc123".into()),
        }],
    }];

    super::enrich_cues_with_attachments(&mut cues, &[], &[], &[], &file_backed_notes);

    assert_eq!(cues[0].content, "[Attachment 1: SK-INFLUX [V MB]_DS_C0919.pdf]");
    assert_eq!(cues[0].attachment_segments.len(), 1);
    assert!(matches!(
        cues[0].attachment_segments[0].kind,
        AttachmentKind::Pdf
    ));
    assert_eq!(
        cues[0].attachment_segments[0].path.as_deref(),
        Some("/tmp/SK-INFLUX [V MB]_DS_C0919.pdf")
    );
    assert_eq!(
        cues[0].attachment_segments[0].sha256.as_deref(),
        Some("abc123")
    );
    assert!(!cues[0].attachment_segments[0].has_ocr);
}

#[test]
fn enrich_cues_with_binary_emits_file_backed_binary_segment() {
    let timestamp = Utc.with_ymd_and_hms(2026, 4, 9, 23, 42, 53).unwrap();
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
    let file_backed_notes = vec![crate::notes::FileBackedNote {
        timestamp: timestamp.timestamp(),
        title: Some("新增備忘錄".into()),
        attachments: vec![crate::notes::FileBackedAttachment {
            kind: AttachmentKind::Binary,
            title: None,
            filename: "34BF703C-DD8C-4607-AA9C-2A71623C2884".into(),
            path: PathBuf::from("/tmp/34BF703C-DD8C-4607-AA9C-2A71623C2884"),
            sha256: Some("def456".into()),
        }],
    }];

    super::enrich_cues_with_attachments(&mut cues, &[], &[], &[], &file_backed_notes);

    assert_eq!(
        cues[0].content,
        "[Attachment 1: 34BF703C-DD8C-4607-AA9C-2A71623C2884]"
    );
    assert_eq!(cues[0].attachment_segments.len(), 1);
    assert!(matches!(
        cues[0].attachment_segments[0].kind,
        AttachmentKind::Binary
    ));
    assert_eq!(
        cues[0].attachment_segments[0].path.as_deref(),
        Some("/tmp/34BF703C-DD8C-4607-AA9C-2A71623C2884")
    );
    assert_eq!(
        cues[0].attachment_segments[0].sha256.as_deref(),
        Some("def456")
    );
    assert!(!cues[0].attachment_segments[0].has_ocr);
}
