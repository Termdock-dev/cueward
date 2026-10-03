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
    let parent_path = receipt
        .request
        .destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent_info = super::super::observe(&receipt.request.root, parent_path, false, None)?;
    let parent = MacFiles.open_directory(Path::new(&parent_info.path))?;
    if parent_info.version != receipt.request.expected_parent_version {
        return Err(FileError::new(
            FileErrorCode::Changed,
            "staging destination parent changed",
        ));
    }
    let root_info = super::super::observe(&receipt.request.root, Path::new("."), false, None)?;
    if MacFiles.stamp(&root.metadata()?).version != root_info.version {
        return Err(unavailable(
            "staging root descriptor does not match selected root",
        ));
    }
    if directory.metadata()?.dev() != parent.metadata()?.dev() {
        let name = format!(".cueward-stage-{}", receipt.operation_id);
        let directory = MacFiles
            .create_directory(&parent, std::ffi::OsStr::new(&name))
            .file?;
        let path = Path::new(&parent_info.path).join(name);
        probe(&directory, &path, guarded_rename::FLAGS)?;
        return Ok(Staging {
            directory,
            path: path.join("staged-object"),
        });
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
