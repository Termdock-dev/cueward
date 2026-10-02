//! Fail-closed same-filesystem publication with no-follow/beneath/exclusive kernel rename.
use super::*;
use std::ffi::{CString, c_char};
use std::fs::File;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;

// Darwin sys/stdio.h: RENAME_EXCL | RENAME_NOFOLLOW_ANY | RENAME_RESOLVE_BENEATH.
const FLAGS: u32 = 0x04 | 0x10 | 0x20;
const ENOTCAPABLE: i32 = 107;
unsafe extern "C" {
    fn renameatx_np(
        from_fd: i32,
        from: *const c_char,
        to_fd: i32,
        to: *const c_char,
        flags: u32,
    ) -> i32;
}

/// Prepare private storage only when the selected root supports guarded atomic publication.
pub(super) fn prepare(receipt: &MutationReceipt, root: &File) -> Result<Staging, FileError> {
    let path = journal::directory(&receipt.operation_id)?;
    if path.join("receipt.json").to_str() != Some(&receipt.receipt_path) {
        return Err(unavailable("private staging receipt path mismatch"));
    }
    let directory = MacFiles.open_directory(&path)?;
    if directory.metadata()?.dev() != root.metadata()?.dev() {
        return Err(unavailable(
            "selected root and private receipt storage must share a filesystem for atomic publication",
        ));
    }
    probe(&directory, &path, FLAGS)?;
    Ok(Staging {
        directory,
        path: path.join("staged-object"),
    })
}

/// Check kernel flag enforcement using only a new private empty object.
pub(super) fn probe(directory: &File, path: &Path, flags: u32) -> Result<(), FileError> {
    let file = tempfile::NamedTempFile::new_in(path)?;
    let name = c_path(Path::new(
        file.path()
            .file_name()
            .ok_or_else(|| unavailable("invalid probe leaf"))?,
    ))?;
    let absolute = c_path(file.path())?;
    // An absolute destination must be rejected by BENEATH. The source and target are
    // the same newly owned empty probe, so ignored flags cannot overwrite user data.
    let result = unsafe {
        renameatx_np(
            directory.as_raw_fd(),
            name.as_ptr(),
            directory.as_raw_fd(),
            absolute.as_ptr(),
            flags,
        )
    };
    let error = std::io::Error::last_os_error();
    if result != -1 || error.raw_os_error() != Some(ENOTCAPABLE) {
        return Err(unavailable(
            "kernel no-follow/beneath/exclusive publication is unavailable; no fallback is permitted",
        ));
    }
    // Drop removes only this newly created empty capability probe, never staging/source data.
    Ok(())
}

/// Publish from private staging using the full relative path rooted at the selected descriptor.
pub(super) fn publish(root: &File, staging: &Staging, destination: &Path) -> Publication {
    let result = rename(root, staging, destination);
    let destination_created = match &result {
        Ok(()) => Some(true),
        Err(error)
            if matches!(
                error.code,
                FileErrorCode::Conflict
                    | FileErrorCode::NotFound
                    | FileErrorCode::PermissionDenied
                    | FileErrorCode::InvalidOptions
                    | FileErrorCode::Unavailable
                    | FileErrorCode::SymlinkDisallowed
            ) =>
        {
            Some(false)
        }
        Err(_) => None,
    };
    Publication {
        result,
        destination_created,
    }
}
fn rename(root: &File, staging: &Staging, destination: &Path) -> Result<(), FileError> {
    let source = c_path(Path::new(
        staging
            .path
            .file_name()
            .ok_or_else(|| unavailable("invalid staging leaf"))?,
    ))?;
    let target = c_path(destination)?;
    // Both names are relative; the kernel resolves from the held staging/root anchors,
    // rejects symlinks/escapes and never overwrites an existing destination.
    if unsafe {
        renameatx_np(
            staging.directory.as_raw_fd(),
            source.as_ptr(),
            root.as_raw_fd(),
            target.as_ptr(),
            FLAGS,
        )
    } == 0
    {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    match error.raw_os_error() {
        Some(22 | 18 | 45 | 102 | ENOTCAPABLE) => Err(unavailable(
            "filesystem cannot perform guarded atomic publication",
        )),
        Some(62) => Err(FileError::new(
            FileErrorCode::SymlinkDisallowed,
            "destination traverses a symlink; nothing published",
        )),
        _ => Err(error.into()),
    }
}
fn c_path(path: &Path) -> Result<CString, FileError> {
    CString::new(path.as_os_str().as_bytes())
        .map_err(|_| FileError::new(FileErrorCode::InvalidOptions, "NUL in publication path"))
}
fn unavailable(message: &str) -> FileError {
    FileError::new(FileErrorCode::Unavailable, message)
}
