use super::*;
use sha2::{Digest, Sha256};
use std::ffi::{CString, c_char, c_void};
use std::fs::File;
use std::os::fd::AsRawFd;

unsafe extern "C" {
    fn flistxattr(fd: i32, buffer: *mut c_char, size: usize, options: i32) -> isize;
    fn fgetxattr(
        fd: i32,
        name: *const c_char,
        value: *mut c_void,
        size: usize,
        position: u32,
        options: i32,
    ) -> isize;
}
const MAX_NAMES: usize = 65536;
const MAX_BYTES: usize = 4 * 1024 * 1024;

/// Hash bounded descriptor attributes including names/lengths, never following a path.
pub(in crate::files) fn digest(file: &File) -> Result<(String, usize), FileError> {
    digest_filtered(file, false).map(|(digest, bytes, _)| (digest, bytes))
}
/// Native trash may add only a zero-byte com.apple.macl marker; still read every bounded value.
pub(in crate::files) fn trash_digest(file: &File) -> Result<(String, usize, bool), FileError> {
    digest_filtered(file, true)
}
fn digest_filtered(
    file: &File,
    allow_empty_macl: bool,
) -> Result<(String, usize, bool), FileError> {
    let names = names(file)?;
    let mut values: Vec<_> = names.split(|b| *b == 0).filter(|s| !s.is_empty()).collect();
    values.sort_unstable();
    let mut hash = Sha256::new();
    let mut total = 0;
    let mut excluded_empty_macl = false;
    for name in values {
        let c_name = CString::new(name).map_err(|_| invalid())?;
        let value = read_value(file, &c_name, MAX_BYTES - total)?;
        let size = value.len();
        total += size;
        if allow_empty_macl && name == b"com.apple.macl" && size == 0 {
            excluded_empty_macl = true;
            continue;
        }
        hash.update((name.len() as u64).to_be_bytes());
        hash.update(name);
        hash.update((size as u64).to_be_bytes());
        hash.update(value);
    }
    if names != self::names(file)? {
        return Err(FileError::new(
            FileErrorCode::Changed,
            "attribute names changed while reading",
        ));
    }
    Ok((format!("{:x}", hash.finalize()), total, excluded_empty_macl))
}

fn read_value(file: &File, name: &CString, maximum: usize) -> Result<Vec<u8>, FileError> {
    // SAFETY: valid borrowed fd and NUL-terminated name; NULL requests length.
    let size = bounded(
        unsafe {
            fgetxattr(
                file.as_raw_fd(),
                name.as_ptr(),
                std::ptr::null_mut(),
                0,
                0,
                0,
            )
        },
        maximum,
    )?;
    let mut value = vec![0u8; size];
    // SAFETY: buffer has size writable bytes; position/options zero.
    let actual = unsafe {
        fgetxattr(
            file.as_raw_fd(),
            name.as_ptr(),
            value.as_mut_ptr().cast(),
            size,
            0,
            0,
        )
    };
    if bounded(actual, size)? != size {
        return Err(FileError::new(
            FileErrorCode::Changed,
            "attribute changed while reading",
        ));
    }
    Ok(value)
}

fn names(file: &File) -> Result<Vec<u8>, FileError> {
    // SAFETY: valid descriptor, NULL buffer requests size.
    let size = bounded(
        unsafe { flistxattr(file.as_raw_fd(), std::ptr::null_mut(), 0, 0) },
        MAX_NAMES,
    )?;
    let mut names = vec![0u8; size];
    // SAFETY: vector owns size writable bytes; zero-size operations are allowed.
    let actual = unsafe { flistxattr(file.as_raw_fd(), names.as_mut_ptr().cast(), size, 0) };
    if bounded(actual, size)? != size || (size > 0 && names.last() != Some(&0)) {
        return Err(invalid());
    }
    Ok(names)
}
fn bounded(value: isize, maximum: usize) -> Result<usize, FileError> {
    if value < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let size = value as usize;
    if size > maximum {
        return Err(FileError::new(
            FileErrorCode::ScanLimit,
            "copy xattrs exceed 64 KiB names or 4 MiB aggregate value budget",
        ));
    }
    Ok(size)
}
fn invalid() -> FileError {
    FileError::new(
        FileErrorCode::Changed,
        "invalid or changed attribute name list",
    )
}
