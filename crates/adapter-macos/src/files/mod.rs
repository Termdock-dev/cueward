//! Scoped file operations with macOS identity, explicit actions and deadlines.
pub mod cloud;
pub mod mutation;
pub mod preview;
pub mod finder;
pub mod spotlight;
mod metadata;
#[cfg(test)]
mod metadata_tests;
mod policy;
mod protocol;
#[cfg(test)]
mod reading_tests;
#[cfg(test)]
mod search_safety_tests;
#[cfg(test)]
mod search_tests;
#[cfg(test)]
mod tests;

use cueward_core::files::*;
use std::fs::{File, Metadata, OpenOptions};
use std::io;
use std::os::macos::fs::MetadataExt as MacMetadataExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

pub use protocol::{MAX_REQUEST_BYTES, run};

struct MacFiles;

impl FilePlatform for MacFiles {
    fn stamp(&self, metadata: &Metadata) -> FileStamp {
        let identity = format!(
            "{}:{}:{}:{}:{}",
            metadata.dev(),
            metadata.ino(),
            metadata.st_gen(),
            metadata.st_birthtime(),
            metadata.st_birthtime_nsec()
        );
        FileStamp {
            version: format!(
                "{identity}:{}:{}:{}:{}:{}:{}:{}",
                metadata.len(),
                metadata.mtime(),
                metadata.mtime_nsec(),
                metadata.ctime(),
                metadata.ctime_nsec(),
                metadata.mode(),
                metadata.st_flags()
            ),
            identity,
            data_state: data_state(metadata.st_flags()),
            mode: Some(metadata.mode()),
        }
    }

    fn open_regular(&self, path: &Path) -> io::Result<File> {
        // Darwin O_NOFOLLOW_ANY guards every component; O_NONBLOCK avoids FIFO waits.
        OpenOptions::new()
            .read(true)
            .custom_flags(0x20000000 | 0x00000004)
            .open(path)
    }

    fn resource_metadata(
        &self,
        path: &Path,
        metadata: &Metadata,
    ) -> Result<ResourceMetadata, FileError> {
        self::metadata::inspect(path, metadata)
    }
}

fn data_state(flags: u32) -> DataState {
    if flags & 0x40000000 != 0 {
        DataState::Dataless
    } else {
        DataState::NotDataless
    }
}

/// Execute inside the short-lived CLI worker, denying dataless materialization.
/// Call `run` from a parent process to enforce a deadline around blocking filesystem calls.
pub fn execute_worker(request: &FileRequest) -> Result<FileResponse, FileError> {
    let _policy = policy::NoMaterialization::enter()?;
    cueward_core::files::execute(&MacFiles, request)
}

/// Observe one scoped path with the shared identity, symlink and version policy.
fn observe(
    root: &Path,
    path: &Path,
    follow_links: bool,
    expected_version: Option<String>,
) -> Result<FileInfo, FileError> {
    let request = FileRequest {
        root: root.to_owned(),
        path: path.to_owned(),
        follow_links,
        expected_version,
        action: FileAction::Info,
    };
    let FileResponse::Info(file) = cueward_core::files::execute(&MacFiles, &request)? else {
        return Err(FileError::new(
            FileErrorCode::Internal,
            "expected file observation",
        ));
    };
    Ok(file)
}
