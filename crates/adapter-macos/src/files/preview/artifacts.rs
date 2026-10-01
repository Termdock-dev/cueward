use super::*;
use crate::files::MacFiles;
use sha2::{Digest, Sha256};
use std::io::Read;

/// Reject inconsistent native pages/budgets and bind each PNG to its actual bytes.
pub(super) fn verify(
    result: &mut native::NativePreview,
    directory: &Path,
    options: &PreviewOptions,
) -> Result<(), FileError> {
    if result.pages.len() > options.page_count {
        return Err(internal("native result exceeds page count"));
    }
    let mut text_bytes = 0;
    let mut preview_bytes = 0;
    for (index, page) in result.pages.iter_mut().enumerate() {
        if page.page != options.start_page + index {
            return Err(internal("native page order/range mismatch"));
        }
        if let ResourceValue::Available { value } = &page.text {
            text_bytes += value.len();
        }
        if page
            .confidence
            .is_some_and(|v| !v.is_finite() || !(0.0..=1.0).contains(&v))
        {
            return Err(internal("invalid OCR confidence"));
        }
        if let ResourceValue::Available { value } = &mut page.preview {
            verify_png(value, page.page, directory, options)?;
            preview_bytes += value.bytes;
        }
    }
    if text_bytes > options.max_text_bytes || preview_bytes > options.max_preview_bytes {
        return Err(internal("native aggregate output exceeds bounds"));
    }
    Ok(())
}

fn verify_png(
    artifact: &mut PreviewArtifact,
    page: usize,
    directory: &Path,
    options: &PreviewOptions,
) -> Result<(), FileError> {
    if artifact.path != format!("page-{page}.png")
        || artifact.format != "png"
        || artifact.width == 0
        || artifact.height == 0
        || artifact.width > options.max_dimension
        || artifact.height > options.max_dimension
        || artifact.bytes > options.max_preview_bytes
    {
        return Err(internal(
            "invalid native artifact name/format/dimensions/size",
        ));
    }
    let path = directory.join(&artifact.path);
    if !std::fs::symlink_metadata(&path)?.is_file() {
        return Err(internal("preview artifact is not a regular file"));
    }
    let mut file = MacFiles.open_regular(&path)?;
    if file.metadata()?.len() != artifact.bytes {
        return Err(internal("preview artifact size differs from native result"));
    }
    artifact.sha256 = hash_png(&mut file, artifact)?;
    artifact.path = path
        .to_str()
        .ok_or_else(|| {
            FileError::new(
                FileErrorCode::UnsupportedPathEncoding,
                "cache artifact path is not UTF-8",
            )
        })?
        .to_owned();
    Ok(())
}
fn internal(message: impl std::fmt::Display) -> FileError {
    FileError::new(FileErrorCode::Internal, message.to_string())
}

fn hash_png(file: &mut std::fs::File, artifact: &PreviewArtifact) -> Result<String, FileError> {
    let mut header = [0u8; 24];
    file.read_exact(&mut header)?;
    if &header[..8] != b"\x89PNG\r\n\x1a\n"
        || &header[12..16] != b"IHDR"
        || u32::from_be_bytes(header[16..20].try_into().map_err(internal)?) as usize
            != artifact.width
        || u32::from_be_bytes(header[20..24].try_into().map_err(internal)?) as usize
            != artifact.height
    {
        return Err(internal(
            "preview PNG header differs from native dimensions",
        ));
    }
    let mut hash = Sha256::new();
    hash.update(header);
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
