//! Fail-closed same-filesystem publication with no-follow/beneath/exclusive kernel rename.
use cueward_core::files::*;
use std::ffi::{CString, c_char};
use std::fs::File;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

// Darwin sys/stdio.h: RENAME_EXCL | RENAME_NOFOLLOW_ANY | RENAME_RESOLVE_BENEATH.
pub(super) const FLAGS: u32 = 0x04 | 0x10 | 0x20;
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

/// Check BENEATH rejection using only a new private empty object.
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

/// Rename relative paths with guarded exclusive resolution, without a fallback.
pub(super) fn rename(
    from: &File,
    source: &Path,
    to: &File,
    target: &Path,
) -> Result<(), FileError> {
    let source = c_path(source)?;
    let target = c_path(target)?;
    if unsafe {
        renameatx_np(
            from.as_raw_fd(),
            source.as_ptr(),
            to.as_raw_fd(),
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
            "filesystem cannot perform guarded exclusive rename; no fallback",
        )),
        Some(62) => Err(FileError::new(
            FileErrorCode::SymlinkDisallowed,
            "rename path traverses a symlink",
        )),
        _ => Err(error.into()),
    }
}
/// Classify errors for which the native operation did not rename an object.
pub(super) fn known_outcome(result: &Result<(), FileError>) -> Option<bool> {
    match result {
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
        _ => None,
    }
}
/// Encode a native path without lossy conversion or embedded NUL.
pub(super) fn c_path(path: &Path) -> Result<CString, FileError> {
    CString::new(path.as_os_str().as_bytes())
        .map_err(|_| FileError::new(FileErrorCode::InvalidOptions, "NUL in publication path"))
}
fn unavailable(message: &str) -> FileError {
    FileError::new(FileErrorCode::Unavailable, message)
}
