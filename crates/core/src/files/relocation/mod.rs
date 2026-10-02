//! Same-filesystem path-based relocation with observations and conservative receipts.
pub mod batch;
mod context;
mod model;
mod validation;
mod receipt;
mod operation;
mod verification;
use crate::files::*;
pub use model::*;
pub use operation::execute;
pub use receipt::*;
use std::fs::{File, Metadata};
use std::path::{Path, PathBuf};
pub use validation::rename_destination;

/// Platform hooks for observations and one guarded rename; no native APIs enter core.
pub trait RelocationPlatform: FilePlatform {
    /// Open a directory without following links or materializing data.
    fn open_directory(&self, path: &Path) -> Result<File, FileError>;
    /// Compare observed filesystem devices, not inferred identity-string formatting.
    fn same_filesystem(&self, source: &Metadata, destination_parent: &Metadata) -> bool;
    /// Establish guarded/exclusive rename support without changing selected objects.
    fn prepare_rename(
        &self,
        _parent: &Path,
        _receipt: &RelocationReceipt,
    ) -> Result<(), FileError> {
        Err(FileError::new(
            FileErrorCode::Unavailable,
            "guarded rename unavailable",
        ))
    }
    /// Submit one root-relative exclusive rename; this is not source-inode CAS.
    fn rename(&self, _root: &File, _request: &RelocationRequest) -> RenameOutcome {
        RenameOutcome {
            result: Err(FileError::new(
                FileErrorCode::Unavailable,
                "guarded rename unavailable",
            )),
            renamed: Some(false),
        }
    }
}

/// Describe a proposed move/rename without changing names, contents or metadata.
pub fn plan(
    platform: &impl RelocationPlatform,
    request: &RelocationRequest,
) -> Result<RelocationPlan, FileError> {
    validation::validate(request)?;
    let context = context::Context::prepare(platform, request)?;
    build_plan(platform, request, &context)
}

fn build_plan<P: RelocationPlatform>(
    platform: &P,
    request: &RelocationRequest,
    context: &context::Context<'_, P>,
) -> Result<RelocationPlan, FileError> {
    let destination_before = context.destination(request)?;
    let destination_path = context.destination_path(request)?;
    let result = RelocationPlan {
        request: request.clone(),
        root: context.root_info.clone(),
        source: context.source.info.clone(),
        source_parent: context.source_parent.info.clone(),
        source_resources: context.resources(platform)?,
        destination_parent: context.destination_parent.info.clone(),
        destination_path,
        destination_before,
        no_op: request.path == request.destination,
        same_filesystem: platform.same_filesystem(
            &context.source.file.metadata()?,
            &context.destination_parent.file.metadata()?,
        ),
    };
    context.revalidate(request)?;
    let after = context.destination(request)?;
    if destination_revision(&after) != destination_revision(&result.destination_before) {
        return Err(FileError::new(
            FileErrorCode::Changed,
            "destination changed while planning",
        ));
    }
    context.revalidate(request)?;
    Ok(result)
}

fn destination_revision(info: &Option<FileInfo>) -> Option<&str> {
    info.as_ref().map(|info| info.version.as_str())
}
