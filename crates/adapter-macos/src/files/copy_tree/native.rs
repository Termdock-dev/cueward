use super::*;
use cueward_core::files::copy_tree::CopyTreePlatform;
use cueward_core::files::mutation::MutationPlatform;
use cueward_core::files::*;
use std::ffi::{CStr, OsString};
use std::fs::File;
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd};
use std::os::unix::ffi::OsStringExt;

impl CopyTreePlatform for MacFiles {
    fn open_tree_link(&self, path: &Path) -> Result<File, FileError> {
        self.open_link(path)
    }
    fn open_tree_directory(&self, path: &Path) -> Result<File, FileError> {
        self.open_directory(path)
    }
    fn tree_names(&self, directory: &File, maximum: usize) -> Result<Vec<OsString>, FileError> {
        // Open a fresh description; dup would share the held descriptor's scan offset.
        // SAFETY: valid borrowed descriptor, constant leaf and read-only flags.
        let fd = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                c".".as_ptr(),
                libc::O_DIRECTORY | libc::O_NOFOLLOW_ANY | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        // SAFETY: openat returned an owned descriptor; fdopendir takes ownership only on success.
        let file = unsafe { File::from_raw_fd(fd) };
        let pointer = unsafe { libc::fdopendir(file.as_raw_fd()) };
        if pointer.is_null() {
            return Err(std::io::Error::last_os_error().into());
        }
        let _ = file.into_raw_fd();
        let stream = Stream(pointer);
        let mut names = Vec::new();
        while let Some(name) = stream.next()? {
            if name == "." || name == ".." {
                continue;
            }
            if names.len() >= maximum {
                return Err(FileError::new(
                    FileErrorCode::ScanLimit,
                    "copy-tree directory exceeds remaining entry budget",
                ));
            }
            names.push(name);
        }
        Ok(names)
    }
    fn validate_tree_metadata(&self, file: &File, info: &FileInfo) -> Result<(), FileError> {
        self.validate_copy_source(file, info)
    }
}
struct Stream(*mut libc::DIR);
impl Stream {
    fn next(&self) -> Result<Option<OsString>, FileError> {
        // SAFETY: stream is uniquely owned and live; errno is thread-local. Copy
        // the bounded name before the next readdir invalidates its borrowed buffer.
        unsafe {
            *libc::__error() = 0;
            let entry = libc::readdir(self.0);
            if entry.is_null() {
                return if *libc::__error() == 0 {
                    Ok(None)
                } else {
                    Err(std::io::Error::last_os_error().into())
                };
            }
            let len = (*entry).d_namlen as usize;
            if len == 0
                || len >= 1024
                || (*entry).d_reclen as usize <= std::mem::offset_of!(libc::dirent, d_name) + len
            {
                return Err(FileError::new(
                    FileErrorCode::Internal,
                    "invalid native directory entry",
                ));
            }
            let bytes = std::slice::from_raw_parts(
                std::ptr::addr_of!((*entry).d_name).cast::<u8>(),
                len + 1,
            );
            let name = CStr::from_bytes_with_nul(bytes)
                .map_err(|_| FileError::new(FileErrorCode::Internal, "invalid directory name"))?;
            if name.to_bytes().contains(&b'/') {
                return Err(FileError::new(
                    FileErrorCode::Internal,
                    "directory name is not a leaf",
                ));
            }
            Ok(Some(OsString::from_vec(name.to_bytes().to_vec())))
        }
    }
}
impl Drop for Stream {
    fn drop(&mut self) {
        // SAFETY: fdopendir succeeded; closedir releases this uniquely owned stream/fd once.
        unsafe {
            libc::closedir(self.0);
        }
    }
}
