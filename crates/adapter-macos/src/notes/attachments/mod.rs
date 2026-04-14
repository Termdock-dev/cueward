mod file_backed;
mod image;
mod map;
mod web_preview;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use cueward_core::{AttachmentKind, AttachmentSegment, Cue, CueSource};

use super::{
    ATTACHMENT_LABEL, AttachmentOcrBlock, FileBackedNote, MEDIA_MATCH_WINDOW_SECS,
    MediaAttachment, MediaNote, MapNote, WebPreviewNote,
};
use file_backed::{build_file_backed_segments, labels_for_file_backed, match_file_backed_note};
use image::{collect_attachment_ocr_blocks, materialize_attachments};
use map::{build_map_segments, labels_for_maps, match_map_note};
use web_preview::{
    build_web_preview_segments, labels_for_web_previews, match_web_preview_note,
};

pub(crate) fn enrich_cues_with_attachments(
    cues: &mut [Cue],
    media_notes: &[MediaNote],
    web_preview_notes: &[WebPreviewNote],
    map_notes: &[MapNote],
    file_backed_notes: &[FileBackedNote],
) {
    for cue in cues.iter_mut() {
        if !matches!(cue.source, CueSource::Notes) {
            continue;
        }

        let placeholder_count = attachment_placeholder_count(&cue.content);
        if placeholder_count == 0 {
            continue;
        }

        let mut labels = Vec::new();
        let mut segments = Vec::new();

        if let Some(media_note) = match_media_note(cue, media_notes) {
            if !media_note.attachments.is_empty() {
                let remaining = placeholder_count.saturating_sub(labels.len());
                let attachments = materialize_attachments(&media_note.attachments, remaining);
                if !attachments.is_empty() {
                    labels.extend(
                        attachments
                            .iter()
                            .map(|attachment| attachment.filename.clone()),
                    );

                    let ocr_blocks = collect_attachment_ocr_blocks(&attachments, remaining);
                    if !ocr_blocks.is_empty() {
                        cue.content = append_attachment_ocr(&cue.content, &ocr_blocks);
                    }

                    segments.extend(build_image_segments(&attachments, Some(&ocr_blocks)));
                }
            }
        }

        if labels.len() < placeholder_count {
            if let Some(map_note) = match_map_note(cue, map_notes) {
                if !map_note.attachments.is_empty() {
                    let remaining = placeholder_count.saturating_sub(labels.len());
                    labels.extend(labels_for_maps(&map_note.attachments, remaining));
                    segments.extend(build_map_segments(
                        &map_note.attachments,
                        segments.len(),
                        remaining,
                    ));
                }
            }
        }

        if labels.len() < placeholder_count {
            if let Some(web_preview_note) = match_web_preview_note(cue, web_preview_notes) {
                if !web_preview_note.attachments.is_empty() {
                    let remaining = placeholder_count.saturating_sub(labels.len());
                    labels.extend(labels_for_web_previews(
                        &web_preview_note.attachments,
                        remaining,
                    ));
                    segments.extend(build_web_preview_segments(
                        &web_preview_note.attachments,
                        segments.len(),
                        remaining,
                    ));
                }
            }
        }

        if labels.len() < placeholder_count {
            if let Some(file_backed_note) = match_file_backed_note(cue, file_backed_notes) {
                if !file_backed_note.attachments.is_empty() {
                    let remaining = placeholder_count.saturating_sub(labels.len());
                    labels.extend(labels_for_file_backed(
                        &file_backed_note.attachments,
                        remaining,
                    ));
                    segments.extend(build_file_backed_segments(
                        &file_backed_note.attachments,
                        segments.len(),
                        remaining,
                    ));
                }
            }
        }

        if !labels.is_empty() {
            cue.content = replace_attachment_labels(&cue.content, &labels, placeholder_count);
        }

        if segments.len() < placeholder_count {
            segments.extend(build_unresolved_segments_from(segments.len(), placeholder_count));
        }

        cue.attachment_segments = segments;
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

        cue.content = replace_attachment_labels(
            &cue.content,
            &media_note
                .attachments
                .iter()
                .map(|attachment| attachment.filename.clone())
                .collect::<Vec<_>>(),
            placeholder_count,
        );

        cue.attachment_segments = build_image_segments(&media_note.attachments, None);
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
        .min_by_key(|note| match_key(cue_title, cue_ts, note.title.as_deref(), note.timestamp))
}

pub(super) fn match_key(
    cue_title: Option<&str>,
    cue_ts: i64,
    note_title: Option<&str>,
    note_ts: i64,
) -> (i32, i64) {
    let title_penalty = match (cue_title, note_title) {
        (Some(cue_title), Some(note_title)) if cue_title == note_title => 0,
        (Some(_), Some(_)) => 1,
        _ => 2,
    };
    (title_penalty, (note_ts - cue_ts).abs())
}

fn replace_attachment_labels(body: &str, labels: &[String], placeholder_count: usize) -> String {
    if placeholder_count == 0 {
        return body.to_string();
    }

    let mut result = body.to_string();
    for (idx, label) in labels.iter().enumerate() {
        if idx >= placeholder_count {
            break;
        }

        let replacement = format!("[Attachment {}: {}]", idx + 1, label);
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

fn build_image_segments(
    attachments: &[MediaAttachment],
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
        .enumerate()
        .map(|(idx, attachment)| {
            let key = attachment_lookup_key(attachment.sha256.as_deref(), &attachment.filename);
            let ocr = ocr_by_attachment.get(&key);
            AttachmentSegment {
                index: idx + 1,
                kind: AttachmentKind::Image,
                title: None,
                url: None,
                latitude: None,
                longitude: None,
                filename: Some(attachment.filename.clone()),
                path: Some(attachment.path.display().to_string()),
                sha256: attachment.sha256.clone(),
                ocr_text: ocr.map(|block| block.text.clone()),
                has_ocr: ocr.is_some(),
            }
        })
        .collect()
}

fn build_unresolved_segments_from(
    existing_count: usize,
    placeholder_count: usize,
) -> Vec<AttachmentSegment> {
    (existing_count..placeholder_count)
        .map(|idx| AttachmentSegment {
            index: idx + 1,
            kind: AttachmentKind::Unresolved,
            title: None,
            url: None,
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

fn attachment_lookup_key(sha256: Option<&str>, filename: &str) -> String {
    sha256.unwrap_or(filename).to_string()
}
