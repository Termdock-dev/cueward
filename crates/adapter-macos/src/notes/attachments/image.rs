use std::fs;
use std::path::PathBuf;

use crate::MacosError;
use crate::ocr;

use super::super::{AttachmentOcrBlock, MediaAttachment, OCR_EMPTY_SENTINEL, home_dir};
use super::super::db::compute_sha256;

pub(super) fn collect_attachment_ocr_blocks(
    attachments: &[MediaAttachment],
    placeholder_count: usize,
) -> Vec<AttachmentOcrBlock> {
    attachments
        .iter()
        .take(placeholder_count)
        .enumerate()
        .filter_map(|(idx, attachment)| {
            let text = load_or_run_attachment_ocr(attachment).ok().flatten()?;
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

fn load_or_run_attachment_ocr(attachment: &MediaAttachment) -> Result<Option<String>, MacosError> {
    let cache_path = if let Some(hash) = attachment.sha256.as_deref() {
        ocr_cache_file_path(hash)?
    } else {
        let sanitized = attachment.filename.replace('/', "_");
        ocr_cache_dir()?.join(format!("name-{sanitized}.txt"))
    };

    if let Ok(cached) = fs::read_to_string(&cache_path) {
        if let Some(cached_text) = cached_ocr_text(&cached) {
            if cached_text.is_empty() {
                return Ok(None);
            }
            return Ok(Some(cached_text));
        }
    }

    let cues = ocr::capture(&attachment.path.to_string_lossy())?;
    let text = cues
        .iter()
        .map(|cue| cue.content.trim())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    fs::create_dir_all(ocr_cache_dir()?)
        .map_err(|err| MacosError::Other(format!("failed to create OCR cache dir: {err}")))?;

    let cached_value = if text.is_empty() {
        OCR_EMPTY_SENTINEL.to_string()
    } else {
        text.clone()
    };

    fs::write(&cache_path, cached_value)
        .map_err(|err| MacosError::Other(format!("failed to write OCR cache: {err}")))?;

    if text.is_empty() {
        Ok(None)
    } else {
        Ok(Some(text))
    }
}

fn cached_ocr_text(cached: &str) -> Option<String> {
    if cached.is_empty() {
        return None;
    }
    if cached.trim() == OCR_EMPTY_SENTINEL {
        return Some(String::new());
    }
    Some(cached.to_string())
}

fn ocr_cache_dir() -> Result<PathBuf, MacosError> {
    Ok(home_dir()?.join(".cueward/cache/ocr"))
}

fn ocr_cache_file_path(hash: &str) -> Result<PathBuf, MacosError> {
    Ok(ocr_cache_dir()?.join(format!("{hash}.txt")))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{cached_ocr_text, ocr_cache_file_path};
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
