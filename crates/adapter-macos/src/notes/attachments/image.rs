use super::super::{AttachmentOcrBlock, MediaAttachment};
use super::super::db::compute_sha256;
use super::ocr_support::load_or_run_file_ocr;

pub(super) fn collect_attachment_ocr_blocks(
    attachments: &[MediaAttachment],
    placeholder_count: usize,
) -> Vec<AttachmentOcrBlock> {
    attachments
        .iter()
        .take(placeholder_count)
        .enumerate()
        .filter_map(|(idx, attachment)| {
            let path = attachment.path.as_path();
            let text = load_or_run_file_ocr(path, attachment.sha256.as_deref(), &attachment.filename)
                .ok()
                .flatten()?;
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return None;
            }

            Some(AttachmentOcrBlock {
                index: idx + 1,
                filename: attachment.filename.clone(),
                sha256: attachment.sha256.clone(),
                text: trimmed.to_string(),
            })
        })
        .collect()
}

pub(super) fn materialize_attachments(
    attachments: &[MediaAttachment],
    placeholder_count: usize,
) -> Vec<MediaAttachment> {
    attachments
        .iter()
        .take(placeholder_count)
        .cloned()
        .map(|mut attachment| {
            if attachment.sha256.is_none() {
                attachment.sha256 = compute_sha256(&attachment.path);
            }
            attachment
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::super::ocr_support::{cached_ocr_text, ocr_cache_file_path};
    use crate::notes::db::compute_sha256;
    use crate::notes::OCR_EMPTY_SENTINEL;

    #[test]
    fn ocr_cache_file_path_uses_hash_name() {
        let path = ocr_cache_file_path("abc123").expect("cache path");

        assert!(path.ends_with(".cueward/cache/ocr/abc123.txt"));
    }

    #[test]
    fn compute_sha256_returns_stable_digest() {
        let temp = tempfile::tempdir().expect("tempdir");
        let target = temp.path().join("image.png");
        fs::write(&target, b"hello world").expect("write media file");

        let digest = compute_sha256(&target);

        assert_eq!(
            digest.as_deref(),
            Some("b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9")
        );
    }

    #[test]
    fn cached_ocr_text_treats_empty_files_as_cache_miss() {
        assert_eq!(cached_ocr_text(""), None);
        assert_eq!(cached_ocr_text(OCR_EMPTY_SENTINEL), Some(String::new()));
    }
}
