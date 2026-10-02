//! Guarded exclusive publication from private same-volume staging.
use super::super::guarded_rename;
use super::*;
use std::fs::File;
use std::os::unix::fs::MetadataExt;
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
    probe(&directory, &path, guarded_rename::FLAGS)?;
    Ok(Staging {
        directory,
        path: path.join("staged-object"),
    })
}

/// Check enforcement using a newly owned empty object only.
pub(super) fn probe(directory: &File, path: &Path, flags: u32) -> Result<(), FileError> {
    guarded_rename::probe(directory, path, flags)
}

/// Publish from private staging using the full relative path rooted at the selected descriptor.
pub(super) fn publish(root: &File, staging: &Staging, destination: &Path) -> Publication {
    let result = rename(root, staging, destination);
    let destination_created = guarded_rename::known_outcome(&result);
    Publication {
        result,
        destination_created,
    }
}
fn rename(root: &File, staging: &Staging, destination: &Path) -> Result<(), FileError> {
    let source = Path::new(
        staging
            .path
            .file_name()
            .ok_or_else(|| unavailable("invalid staging leaf"))?,
    );
    guarded_rename::rename(&staging.directory, source, root, destination)
}
fn unavailable(message: &str) -> FileError {
    FileError::new(FileErrorCode::Unavailable, message)
}
