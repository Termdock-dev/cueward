//! Hold only a leaf symlink after a no-follow parent open, without opening its target.
use super::*;
use crate::files::guarded_rename::c_path;
use std::os::fd::{AsRawFd, FromRawFd};

pub(super) fn open(path: &Path) -> Result<File, FileError> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid("missing link parent"))?;
    let leaf = path
        .file_name()
        .ok_or_else(|| invalid("missing link leaf"))?;
    let directory = RelocationPlatform::open_directory(&MacFiles, parent)?;
    let leaf = c_path(Path::new(leaf))?;
    // O_SYMLINK opens the link object, including broken/outside targets. Combining
    // O_NOFOLLOW[_ANY] with it rejects a leaf link on Darwin, so only the already
    // guarded parent descriptor is traversed here. O_NONBLOCK avoids FIFO waits
    // if an external actor replaces the leaf; fstat rejects every non-link below.
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            leaf.as_ptr(),
            libc::O_RDONLY | libc::O_SYMLINK | libc::O_CLOEXEC | libc::O_NONBLOCK,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    // SAFETY: openat returned a newly owned descriptor; File closes it on failure too.
    let file = unsafe { File::from_raw_fd(fd) };
    if !file.metadata()?.file_type().is_symlink() {
        return Err(FileError::new(
            FileErrorCode::Changed,
            "selected link was replaced",
        ));
    }
    Ok(file)
}
fn invalid(message: &str) -> FileError {
    FileError::new(FileErrorCode::InvalidOptions, message)
}
