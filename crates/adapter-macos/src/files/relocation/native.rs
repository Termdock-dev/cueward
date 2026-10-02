use super::super::{guarded_rename, metadata, store::Store};
use super::*;
use objc2::rc::autoreleasepool;
use objc2_foundation::{NSString, NSURL, NSURLVolumeSupportsExclusiveRenamingKey};

/// Require a declared exclusive-rename volume and kernel guarded-resolution support.
pub(super) fn prepare(parent: &Path, receipt: &RelocationReceipt) -> Result<(), FileError> {
    let text = parent.to_str().ok_or_else(|| {
        FileError::new(
            FileErrorCode::UnsupportedPathEncoding,
            "parent must be UTF-8",
        )
    })?;
    let supported = autoreleasepool(|_| {
        let url = NSURL::fileURLWithPath_isDirectory(&NSString::from_str(text), true);
        unsafe {
            metadata::read(
                &url,
                NSURLVolumeSupportsExclusiveRenamingKey,
                metadata::boolean,
            )
        }
    });
    if !matches!(supported, ResourceValue::Available { value: true }) {
        return Err(FileError::new(
            FileErrorCode::Unavailable,
            format!("exclusive rename support unavailable: {supported:?}"),
        ));
    }
    let path = Store("files-relocation").directory(&receipt.operation_id)?;
    if path.join("receipt.json").to_str() != Some(&receipt.receipt_path) {
        return Err(FileError::new(
            FileErrorCode::Internal,
            "relocation receipt path mismatch",
        ));
    }
    let directory = RelocationPlatform::open_directory(&MacFiles, &path)?;
    guarded_rename::probe(&directory, &path, guarded_rename::FLAGS)
}
/// Root-relative native path operation, deliberately not source-inode CAS.
pub(super) fn rename(root: &File, request: &RelocationRequest) -> relocation::RenameOutcome {
    let result = guarded_rename::rename(root, &request.path, root, &request.destination);
    let renamed = guarded_rename::known_outcome(&result);
    relocation::RenameOutcome { result, renamed }
}
