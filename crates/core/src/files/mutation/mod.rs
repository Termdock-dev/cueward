//! No-overwrite mkdir/single-file copy with durable checkpoints and explicit outcomes.
mod context;
mod copying;
mod model;
mod staging;
use crate::files::*;
pub use model::*;
use std::ffi::OsStr;
use std::fs::File;
use std::path::{Path, PathBuf};

/// A private staging creation error can occur after mkdir but before opening its descriptor.
pub struct Creation {
    pub file: Result<File, FileError>,
    pub destination_created: Option<bool>,
}

/// Private staging storage remains recoverable when publication fails.
pub struct Staging {
    pub directory: File,
    pub path: PathBuf,
}

/// Atomic publication can fail without delivery, or return an uncertain filesystem error.
pub struct Publication {
    pub result: Result<(), FileError>,
    pub destination_created: Option<bool>,
}

/// Guarded descriptor-relative creation and native metadata hooks; no platform API in core.
pub trait MutationPlatform: FilePlatform {
    /// Prepare private same-filesystem storage and fail closed without guarded publication.
    fn prepare_staging(&self, receipt: &MutationReceipt, root: &File)
    -> Result<Staging, FileError>;
    /// Publish a fully prepared object beneath the root descriptor; never use a cached parent.
    fn publish(&self, root: &File, staging: &Staging, destination: &Path) -> Publication;
    /// Open a directory without following any symlink component.
    fn open_directory(&self, path: &Path) -> Result<File, FileError>;
    /// Exclusively create a new read/write file in the private staging directory.
    fn create_file(&self, parent: &File, name: &OsStr) -> Creation;
    /// Create one private staging directory (not parents) and return its descriptor.
    fn create_directory(&self, parent: &File, name: &OsStr) -> Creation;
    /// Reject unsupported alias/security/metadata cases before creating a destination.
    fn validate_copy_source(&self, file: &File, info: &FileInfo) -> Result<(), FileError>;
    /// Copy and verify bounded extended attributes; return their digest and aggregate bytes.
    fn copy_attributes(
        &self,
        source: &File,
        destination: &File,
    ) -> Result<(String, usize), FileError>;
}

/// Perform one mutation, checkpointing before writes. The caller finalizes errors durably.
pub fn execute(
    platform: &impl MutationPlatform,
    receipt: &mut MutationReceipt,
    checkpoint: &mut impl FnMut(&MutationReceipt) -> Result<(), FileError>,
) -> Result<(), FileError> {
    let context = context::prepare(platform, receipt)?;
    context.revalidate_before(receipt)?;
    let (staging, mut destination) = staging::prepare(platform, &context, receipt, checkpoint)?;
    staging::populate(platform, &context, &mut destination, receipt, checkpoint)?;
    context.revalidate_before(receipt)?;
    receipt.stage = MutationStage::Creating;
    receipt.mutation_attempted = true;
    checkpoint(receipt)?;
    let publication = platform.publish(&context.root, &staging, &receipt.request.destination);
    receipt.destination_created = publication.destination_created;
    publication.result?;
    receipt.destination_created_observation = Some(context.destination_info(&destination)?);
    checkpoint(receipt)?;
    receipt.stage = MutationStage::Verifying;
    checkpoint(receipt)?;
    context.finish(&destination, receipt)?;
    receipt.stage = MutationStage::Finished;
    receipt.status = MutationStatus::Completed;
    receipt.completion_verified = true;
    checkpoint(receipt)
}

/// Record a returned error without implying that any partial destination was removed.
pub fn record_error(receipt: &mut MutationReceipt, error: FileError) {
    receipt.status = match receipt.destination_created {
        Some(true) => MutationStatus::Incomplete,
        Some(false) => MutationStatus::NotStarted,
        None if !receipt.mutation_attempted => MutationStatus::NotStarted,
        None => MutationStatus::Uncertain,
    };
    receipt.completion_verified = false;
    receipt.error = Some(error);
}
