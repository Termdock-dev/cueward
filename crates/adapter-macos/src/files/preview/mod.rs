//! Bounded PDF/image/Quick Look previews from guarded private source snapshots.
mod artifacts;
mod model;
mod native;
mod snapshot;
#[cfg(test)]
mod tests;

use super::{observe, policy, protocol};
use cueward_core::files::*;
pub use model::*;
use std::path::{Path, PathBuf};

/// Supervise a preview and retain only verified PNG artifacts on success.
pub fn run(
    executable: &Path,
    request: &PreviewRequest,
    timeout_ms: u64,
) -> Result<PreviewResponse, FileError> {
    validate(request)?;
    let directory = operation_directory()?;
    let worker = PreviewWorkerRequest {
        request: request.clone(),
        directory: directory.path().to_owned(),
    };
    let response = protocol::run_typed(executable, "files-preview-worker", &worker, timeout_ms)?;
    let PreviewResponse::Preview(value) = &response;
    if value.cache_directory.is_some() {
        let _ = directory.keep();
    }
    Ok(response)
}

fn cache_root() -> Result<PathBuf, FileError> {
    let home = std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            FileError::new(
                FileErrorCode::Unavailable,
                "HOME is not set for preview cache",
            )
        })?;
    let path = PathBuf::from(home).join(".cueward/cache/file-previews");
    std::fs::create_dir_all(&path)?;
    Ok(path.canonicalize()?)
}

fn operation_directory() -> Result<tempfile::TempDir, FileError> {
    use std::os::unix::fs::PermissionsExt;
    Ok(tempfile::Builder::new()
        .prefix("preview-")
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir_in(cache_root()?)?)
}

/// Execute only in a supervised worker; the parent owns directory cleanup/deadline.
pub fn execute_worker(worker: &PreviewWorkerRequest) -> Result<PreviewResponse, FileError> {
    validate(&worker.request)?;
    validate_directory(&worker.directory)?;
    let _policy = policy::NoMaterialization::enter()?;
    execute(worker, native::inspect)
}

fn validate_directory(directory: &Path) -> Result<(), FileError> {
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::symlink_metadata(directory)?;
    if !metadata.is_dir()
        || metadata.mode() & 0o777 != 0o700
        || directory.parent() != Some(cache_root()?.as_path())
        || !directory
            .file_name()
            .is_some_and(|s| s.to_string_lossy().starts_with("preview-"))
        || std::fs::read_dir(directory)?.next().is_some()
    {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "worker requires an empty private preview operation directory",
        ));
    }
    Ok(())
}

fn validate(request: &PreviewRequest) -> Result<(), FileError> {
    let o = &request.options;
    if request.path.as_os_str().is_empty()
        || !(1..=256 * 1024 * 1024).contains(&o.max_input_bytes)
        || !(1..=1024 * 1024).contains(&o.max_text_bytes)
        || !(1..=2048).contains(&o.max_dimension)
        || !(1..=32 * 1024 * 1024).contains(&o.max_preview_bytes)
        || !(1..=1_000_000).contains(&o.start_page)
        || !(1..=20).contains(&o.page_count)
    {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "invalid preview path/page/byte/pixel limits",
        ));
    }
    if o.kind != PreviewKind::Pdf && (o.start_page != 1 || o.page_count != 1)
        || o.kind == PreviewKind::Thumbnail && (o.ocr || !o.render)
    {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "page ranges apply only to PDF; thumbnails require rendering without OCR",
        ));
    }
    Ok(())
}

fn execute(
    worker: &PreviewWorkerRequest,
    inspect: impl FnOnce(&Path, &Path, &PreviewOptions) -> Result<native::NativePreview, FileError>,
) -> Result<PreviewResponse, FileError> {
    let request = &worker.request;
    let root = observe(&request.root, Path::new("."), false, None)?;
    let file = observe(
        &request.root,
        &request.path,
        request.follow_links,
        request.expected_version.clone(),
    )?;
    if request.options.kind == PreviewKind::Thumbnail {
        require_non_alias(&file)?;
    }
    let (input, source_sha256) =
        snapshot::create(&file, &worker.directory, request.options.max_input_bytes)?;
    revalidate(request, &file, &root)?;
    let mut result = inspect(&input, &worker.directory, &request.options)?;
    artifacts::verify(&mut result, &worker.directory, &request.options)?;
    revalidate(request, &file, &root)?;
    std::fs::remove_file(input)?;
    let has_artifacts = result
        .pages
        .iter()
        .any(|p| matches!(p.preview, ResourceValue::Available { .. }));
    let text_truncated = result.pages.iter().any(|p| p.text_truncated);
    Ok(PreviewResponse::Preview(Box::new(FilePreview {
        file,
        source_sha256,
        kind: request.options.kind,
        total_pages: result.total_pages,
        pdf_encrypted: result.pdf_encrypted,
        image: result.image,
        pages: result.pages,
        selection_truncated: result.selection_truncated,
        text_truncated,
        cache_directory: has_artifacts.then(|| worker.directory.to_string_lossy().into_owned()),
    })))
}

fn require_non_alias(file: &FileInfo) -> Result<(), FileError> {
    let path = Path::new(&file.path);
    let resources = super::metadata::inspect(path, &std::fs::symlink_metadata(path)?)?;
    match resources.is_alias_file {
        ResourceValue::Available { value: false } => Ok(()),
        ResourceValue::Available { value: true } => Err(FileError::new(
            FileErrorCode::UnsupportedType,
            "Quick Look alias targets are not resolved",
        )),
        _ => Err(FileError::new(
            FileErrorCode::Unavailable,
            "cannot verify that Quick Look source is not an alias",
        )),
    }
}

fn revalidate(request: &PreviewRequest, file: &FileInfo, root: &FileInfo) -> Result<(), FileError> {
    let after = observe(
        &request.root,
        &request.path,
        request.follow_links,
        Some(file.version.clone()),
    )?;
    let current_root = observe(
        &request.root,
        Path::new("."),
        false,
        Some(root.version.clone()),
    )?;
    if file.path != after.path || root.path != current_root.path {
        return Err(FileError::new(
            FileErrorCode::Changed,
            "preview source or root resolution changed",
        ));
    }
    Ok(())
}
