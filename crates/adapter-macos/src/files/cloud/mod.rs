//! iCloud resource observations and explicit bounded asynchronous submissions.
mod model;
mod native;
#[cfg(test)]
mod tests;

use super::{observe, policy, protocol};
use cueward_core::files::*;
pub use model::*;
use std::path::{Path, PathBuf};

trait Native {
    fn inspect(&self, file: &FileInfo) -> Result<CloudResources, FileError>;
    fn download(&self, file: &FileInfo) -> Result<(), FileError>;
}
struct System;
impl Native for System {
    fn inspect(&self, file: &FileInfo) -> Result<CloudResources, FileError> {
        native::inspect(file)
    }
    fn download(&self, file: &FileInfo) -> Result<(), FileError> {
        native::download(file)
    }
}

/// Supervise one cloud worker; a failed deadline cannot cancel provider activity.
pub fn run(
    executable: &Path,
    request: &CloudRequest,
    timeout_ms: u64,
) -> Result<CloudResponse, FileError> {
    let mut result = protocol::run_typed(executable, "files-cloud-worker", request, timeout_ms);
    if matches!(request.action, CloudAction::Download { .. }) {
        if let Err(error) = &mut result {
            if matches!(
                error.code,
                FileErrorCode::Timeout | FileErrorCode::Io | FileErrorCode::Internal
            ) {
                error.message.push_str("; download may already have been requested and can continue; inspect cloud status before retrying");
            }
        }
    }
    result
}

/// Observe metadata under no-materialization; only explicit download calls the API.
pub fn execute_worker(request: &CloudRequest) -> Result<CloudResponse, FileError> {
    let _policy = policy::NoMaterialization::enter()?;
    execute(request, &System, None)
}

fn execute(
    request: &CloudRequest,
    native: &impl Native,
    lock_dir: Option<&Path>,
) -> Result<CloudResponse, FileError> {
    validate(request)?;
    let root = observe(&request.root, Path::new("."), false, None)?;
    match request.action {
        CloudAction::Status => {
            let value = status(request, native)?;
            check_root(request, &root)?;
            Ok(CloudResponse::CloudStatus(Box::new(value)))
        }
        CloudAction::Download { max_bytes } => download(request, root, max_bytes, native, lock_dir),
    }
}

fn validate(request: &CloudRequest) -> Result<(), FileError> {
    if let CloudAction::Download { max_bytes } = request.action {
        if !(1..=256 * 1024 * 1024).contains(&max_bytes)
            || request
                .expected_version
                .as_ref()
                .is_none_or(String::is_empty)
        {
            return Err(FileError::new(
                FileErrorCode::InvalidOptions,
                "download requires expected-version and max-bytes 1..268435456",
            ));
        }
    }
    Ok(())
}

fn status(request: &CloudRequest, native: &impl Native) -> Result<CloudStatus, FileError> {
    let before = observe(
        &request.root,
        &request.path,
        request.follow_links,
        request.expected_version.clone(),
    )?;
    let resources = if matches!(before.kind, FileKind::File | FileKind::Directory) {
        native.inspect(&before)?
    } else {
        native::skipped(native::Skip::NotApplicable)
    };
    let after = observe(
        &request.root,
        &request.path,
        request.follow_links,
        Some(before.version.clone()),
    )?;
    if before.path != after.path {
        return Err(FileError::new(
            FileErrorCode::Changed,
            "cloud path changed during observation",
        ));
    }
    Ok(CloudStatus {
        file: before,
        resources,
        provider_coverage: ProviderCoverage::IcloudKeysOtherProvidersUnknown,
        download_requested_by_operation: false,
    })
}

fn check_root(request: &CloudRequest, root: &FileInfo) -> Result<(), FileError> {
    let after = observe(
        &request.root,
        Path::new("."),
        false,
        Some(root.version.clone()),
    )?;
    if after.path != root.path {
        return Err(FileError::new(
            FileErrorCode::Changed,
            "cloud root path changed",
        ));
    }
    Ok(())
}

fn download(
    request: &CloudRequest,
    root: FileInfo,
    max_bytes: u64,
    native: &impl Native,
    directory: Option<&Path>,
) -> Result<CloudResponse, FileError> {
    let _lock = lock(directory)?;
    let before = status(request, native)?;
    check_root(request, &root)?;
    let existing = preflight(&before, max_bytes)?;
    let operation_id = uuid::Uuid::new_v4().to_string();
    let (disposition, submitted) = match existing {
        Some(disposition) => (disposition, false),
        None => {
            submit(request, &before, native)?;
            (DownloadDisposition::SentUnverified, true)
        }
    };
    let post_check =
        check_root(request, &root).and_then(|()| post_status(request, &before, native));
    Ok(CloudResponse::CloudDownload(Box::new(CloudDownload {
        operation_id,
        status: disposition,
        before,
        max_bytes,
        download_requested_by_operation: submitted,
        completion_verified: false,
        post_check,
    })))
}

fn lock(directory: Option<&Path>) -> Result<std::fs::File, FileError> {
    let directory = match directory {
        Some(path) => path.to_owned(),
        None => PathBuf::from(
            crate::screenshot::ensure_cache_dir()
                .map_err(|e| FileError::new(FileErrorCode::Unavailable, e.to_string()))?,
        ),
    };
    let lock =
        crate::window::lock_path(&directory.join("files-cloud-download.lock")).map_err(|e| {
            FileError::new(
                FileErrorCode::Unavailable,
                format!("download not submitted: {e}"),
            )
        })?;
    Ok(lock)
}

fn submit(
    request: &CloudRequest,
    before: &CloudStatus,
    native: &impl Native,
) -> Result<(), FileError> {
    let current = observe(
        &request.root,
        &request.path,
        request.follow_links,
        Some(before.file.version.clone()),
    )?;
    if current.path != before.file.path {
        return Err(FileError::new(
            FileErrorCode::Changed,
            "download path changed before submission",
        ));
    }
    native.download(&before.file)
}

fn post_status(
    request: &CloudRequest,
    before: &CloudStatus,
    native: &impl Native,
) -> Result<CloudStatus, FileError> {
    let mut request = request.clone();
    request.expected_version = None;
    let after = status(&request, native)?;
    if after.file.identity != before.file.identity || after.file.path != before.file.path {
        return Err(FileError::new(
            FileErrorCode::Changed,
            "download item identity/path changed after submission",
        ));
    }
    Ok(after)
}

fn preflight(
    before: &CloudStatus,
    max_bytes: u64,
) -> Result<Option<DownloadDisposition>, FileError> {
    require_downloadable(before, max_bytes)?;
    if matches!(
        before.resources.downloading_status,
        ResourceValue::Available {
            value: DownloadingStatus::Current
        }
    ) && before.file.data_state == DataState::NotDataless
    {
        return Ok(Some(DownloadDisposition::AlreadyCurrent));
    }
    if matches!(
        before.resources.is_downloading,
        ResourceValue::Available { value: true }
    ) {
        return Ok(Some(DownloadDisposition::AlreadyRequested));
    }
    Ok(None)
}

fn require_downloadable(before: &CloudStatus, max_bytes: u64) -> Result<(), FileError> {
    if before.file.kind != FileKind::File {
        return Err(FileError::new(
            FileErrorCode::UnsupportedType,
            "download accepts one regular file; directories and unfollowed links are unsupported",
        ));
    }
    if before.file.size > max_bytes {
        return Err(FileError::new(
            FileErrorCode::ScanLimit,
            "reported file size exceeds max-bytes; download was not requested",
        ));
    }
    if !matches!(
        before.resources.is_ubiquitous,
        ResourceValue::Available { value: true }
    ) {
        return Err(FileError::new(
            FileErrorCode::UnsupportedType,
            "iCloud membership must be explicitly available/true; other provider downloads are unsupported",
        ));
    }
    Ok(())
}
