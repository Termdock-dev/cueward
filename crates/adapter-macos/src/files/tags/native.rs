//! Read and edit bounded tags on the held inode, with observed revision guards.
use super::*;
use std::ffi::{c_char, c_void};
use std::os::fd::AsRawFd;
const NAME: &std::ffi::CStr = c"com.apple.metadata:_kMDItemUserTags";
unsafe extern "C" {
    fn fgetxattr(
        fd: i32,
        name: *const c_char,
        value: *mut c_void,
        size: usize,
        position: u32,
        options: i32,
    ) -> isize;
    fn fsetxattr(
        fd: i32,
        name: *const c_char,
        value: *const c_void,
        size: usize,
        position: u32,
        options: i32,
    ) -> i32;
}
/// Query a bounded attribute twice; absence is explicit, unknown state is never empty.
pub(super) fn read(file: &File) -> Result<codec::Stored, FileError> {
    codec::Stored::decode(read_attribute(file, NAME)?)
}

fn read_attribute(file: &File, name: &std::ffi::CStr) -> Result<Option<Vec<u8>>, FileError> {
    // SAFETY: valid borrowed fd/name; null requests size, no materialization flags.
    let size = unsafe {
        fgetxattr(
            file.as_raw_fd(),
            name.as_ptr(),
            std::ptr::null_mut(),
            0,
            0,
            0,
        )
    };
    if size < 0 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(93) {
            return Ok(None);
        } // Darwin ENOATTR.
        return Err(error.into());
    }
    if size > 65536 {
        return Err(FileError::new(
            FileErrorCode::ScanLimit,
            "tag attribute exceeds 64 KiB",
        ));
    }
    let mut bytes = vec![0; size as usize];
    // SAFETY: writable vector owns its advertised capacity; position/options zero.
    let actual = unsafe {
        fgetxattr(
            file.as_raw_fd(),
            name.as_ptr(),
            bytes.as_mut_ptr().cast(),
            bytes.len(),
            0,
            0,
        )
    };
    if actual < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    if actual != size {
        return Err(changed("tag attribute changed while reading"));
    }
    Ok(Some(bytes))
}
/// Create or replace one attribute; caller rechecks revision and saves original bytes first.
pub(super) fn write(file: &File, bytes: &[u8], existed: bool) -> Result<(), FileError> {
    // SAFETY: valid descriptor/name and bounded readable payload. CREATE/REPLACE conditions on presence, not atomic value revision.
    let result = unsafe {
        fsetxattr(
            file.as_raw_fd(),
            NAME.as_ptr(),
            bytes.as_ptr().cast(),
            bytes.len(),
            0,
            if existed { 4 } else { 2 }, // Darwin XATTR_REPLACE / XATTR_CREATE.
        )
    };
    if result != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

/// Detect legacy Finder label fallback without changing FinderInfo or inventing tags.
pub(super) fn has_legacy_label(file: &File) -> Result<bool, FileError> {
    let Some(bytes) = read_attribute(file, c"com.apple.FinderInfo")? else {
        return Ok(false);
    };
    if bytes.len() != 32 {
        return Err(FileError::new(
            FileErrorCode::UnsupportedType,
            "unsupported FinderInfo shape",
        ));
    }
    // Finder flags are a big-endian u16 at offset 8; kColor mask is 0x000e.
    Ok(bytes[9] & 0x0e != 0)
}
