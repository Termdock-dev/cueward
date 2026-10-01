use super::*;
use std::ffi::{CString, OsStr};
use std::fs::{File, OpenOptions};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::macos::fs::MetadataExt as MacMetadataExt;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

unsafe extern "C" {
    fn openat(directory: i32, path: *const std::ffi::c_char, flags: i32, ...) -> i32;
    fn mkdirat(directory: i32, path: *const std::ffi::c_char, mode: u16) -> i32;
    fn fcopyfile(source: i32, destination: i32, state: *mut std::ffi::c_void, flags: u32) -> i32;
}
const DIRECTORY: i32 = 0x00100000;
const NOFOLLOW: i32 = 0x20000000;
const CLOEXEC: i32 = 0x01000000;
const NONBLOCK: i32 = 4;

impl MutationPlatform for MacFiles {
    fn open_directory(&self, path: &Path) -> Result<File, FileError> {
        Ok(OpenOptions::new()
            .read(true)
            .custom_flags(DIRECTORY | NOFOLLOW | NONBLOCK | CLOEXEC)
            .open(path)?)
    }
    fn create_file(&self, parent: &File, name: &OsStr) -> Creation {
        let name = match leaf(name) {
            Ok(name) => name,
            Err(error) => return creation(Err(error), Some(false)),
        };
        // SAFETY: valid parent descriptor and NUL-terminated leaf; O_CREAT receives mode_t.
        let fd = unsafe {
            openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                2 | 0x200 | 0x800 | NOFOLLOW | NONBLOCK | CLOEXEC,
                0o600u32,
            )
        };
        creation(descriptor(fd), None)
    }
    fn create_directory(&self, parent: &File, name: &OsStr) -> Creation {
        let name = match leaf(name) {
            Ok(name) => name,
            Err(error) => return creation(Err(error), Some(false)),
        };
        // SAFETY: mkdirat only creates this validated single leaf; mode_t is Darwin u16.
        if unsafe { mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) } != 0 {
            return creation(Err(std::io::Error::last_os_error().into()), None);
        }
        // SAFETY: valid leaf/parent; openat without O_CREAT has no mode argument.
        creation(
            descriptor(unsafe {
                openat(
                    parent.as_raw_fd(),
                    name.as_ptr(),
                    DIRECTORY | NOFOLLOW | NONBLOCK | CLOEXEC,
                )
            }),
            Some(true),
        )
    }
    fn validate_copy_source(&self, file: &File, info: &FileInfo) -> Result<(), FileError> {
        let metadata = file.metadata()?;
        if metadata.mode() & 0o7000 != 0 || metadata.st_flags() & 0x20 != 0 {
            return Err(FileError::new(
                FileErrorCode::UnsupportedType,
                "special permission bits and compressed sources are not supported in this copy slice",
            ));
        }
        let resources = super::super::metadata::inspect(Path::new(&info.path), &metadata)?;
        match resources.is_alias_file {
            ResourceValue::Available { value: false } => {}
            ResourceValue::Available { value: true } => {
                return Err(FileError::new(
                    FileErrorCode::UnsupportedType,
                    "Finder aliases are not copied or resolved",
                ));
            }
            _ => {
                return Err(FileError::new(
                    FileErrorCode::Unavailable,
                    "cannot verify that copy source is not a Finder alias",
                ));
            }
        }
        attributes::digest(file)?;
        Ok(())
    }
    fn copy_attributes(
        &self,
        source: &File,
        destination: &File,
    ) -> Result<(String, usize), FileError> {
        let before = attributes::digest(source)?;
        // SAFETY: borrowed valid descriptors; COPYFILE_XATTR=4, NULL default state, no data/ACL/stat flags.
        if unsafe {
            fcopyfile(
                source.as_raw_fd(),
                destination.as_raw_fd(),
                std::ptr::null_mut(),
                4,
            )
        } != 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
        if before != attributes::digest(source)? || before != attributes::digest(destination)? {
            return Err(FileError::new(
                FileErrorCode::VerificationFailed,
                "extended attributes changed or did not match after copying",
            ));
        }
        Ok(before)
    }
}

fn leaf(name: &OsStr) -> Result<CString, FileError> {
    let bytes = name.as_bytes();
    if bytes.is_empty() || bytes == b"." || bytes == b".." || bytes.contains(&b'/') {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "creation requires one normal leaf name",
        ));
    }
    CString::new(bytes)
        .map_err(|_| FileError::new(FileErrorCode::InvalidOptions, "NUL in leaf name"))
}
fn descriptor(fd: i32) -> Result<File, FileError> {
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    // SAFETY: successful openat returns a new uniquely owned descriptor.
    Ok(unsafe { File::from_raw_fd(fd) })
}
fn creation(file: Result<File, FileError>, known: Option<bool>) -> Creation {
    let destination_created = if file.is_ok() {
        Some(true)
    } else if known.is_some() {
        known
    } else if file.as_ref().is_err_and(|e| {
        matches!(
            e.code,
            FileErrorCode::Conflict | FileErrorCode::PermissionDenied | FileErrorCode::NotFound
        )
    }) {
        Some(false)
    } else {
        None
    };
    Creation {
        file,
        destination_created,
    }
}
