use std::fs;
use std::path::Path;
use std::path::PathBuf;

use crate::MacosError;
use crate::ocr;

use super::super::{OCR_EMPTY_SENTINEL, home_dir};

pub(super) fn load_or_run_file_ocr(
    path: &Path,
    sha256: Option<&str>,
    filename: &str,
) -> Result<Option<String>, MacosError> {
    let cache_path = if let Some(hash) = sha256 {
        ocr_cache_file_path(hash)?
    } else {
        let sanitized = filename.replace('/', "_");
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

    let cues = ocr::capture(&path.to_string_lossy())?;
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

pub(super) fn cached_ocr_text(cached: &str) -> Option<String> {
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

pub(super) fn ocr_cache_file_path(hash: &str) -> Result<PathBuf, MacosError> {
    Ok(ocr_cache_dir()?.join(format!("{hash}.txt")))
}

#[cfg(test)]
mod tests {
    use super::{cached_ocr_text, ocr_cache_file_path};
    use crate::notes::OCR_EMPTY_SENTINEL;

    #[test]
    fn ocr_cache_file_path_uses_hash_name() {
        let path = ocr_cache_file_path("abc123").expect("cache path");

        assert!(path.ends_with(".cueward/cache/ocr/abc123.txt"));
    }

    #[test]
    fn cached_ocr_text_treats_empty_files_as_cache_miss() {
        assert_eq!(cached_ocr_text(""), None);
        assert_eq!(cached_ocr_text(OCR_EMPTY_SENTINEL), Some(String::new()));
    }
}
