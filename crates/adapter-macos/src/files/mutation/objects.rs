//! Descriptor-relative symlink creation; neither reference targets nor parent paths are followed.
use super::*;
use std::ffi::{CString, OsStr};
use std::fs::File;
use std::os::fd::{AsRawFd, FromRawFd};

pub(super) fn create_link(parent: &File, name: &OsStr, target: &str) -> Creation {
    let result = (|| {
        let leaf = native::leaf(name)?;
        let target = CString::new(target).map_err(|_| {
            FileError::new(FileErrorCode::InvalidOptions, "NUL in symlink reference")
        })?;
        // SAFETY: bounded NUL-terminated strings, held directory and one validated leaf.
        if unsafe { libc::symlinkat(target.as_ptr(), parent.as_raw_fd(), leaf.as_ptr()) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                leaf.as_ptr(),
                libc::O_SYMLINK | libc::O_CLOEXEC | libc::O_NONBLOCK,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        // SAFETY: successful openat returned one owned fd. Refuse a replaced non-link.
        let file = unsafe { File::from_raw_fd(fd) };
        if !file.metadata()?.file_type().is_symlink() {
            return Err(FileError::new(
                FileErrorCode::Changed,
                "created link was replaced",
            ));
        }
        Ok(file)
    })();
    Creation {
        destination_created: if result.is_ok() { Some(true) } else { None },
        file: result,
    }
}
