//! No-overwrite mkdir/single-file copy with durable checkpoints and explicit outcomes.
mod context;
mod copying;
mod model;
use crate::files::*;
pub use model::*;
use std::ffi::OsStr;
use std::fs::File;
use std::path::Path;

/// A creation error can occur after mkdir succeeded but before its descriptor opened.
pub struct Creation {
    pub file: Result<File, FileError>,
    pub destination_created: Option<bool>,
}

/// Guarded descriptor-relative creation and native metadata hooks; no platform API in core.
pub trait MutationPlatform: FilePlatform {
    /// Open a directory without following any symlink component.
    fn open_directory(&self, path: &Path) -> Result<File, FileError>;
    /// Atomically create a new read/write file relative to an opened parent, never replacing.
    fn create_file(&self, parent: &File, name: &OsStr) -> Creation;
    /// Create one directory (not parents) and return its opened descriptor.
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
    receipt.stage = MutationStage::Creating;
    receipt.mutation_attempted = true;
    checkpoint(receipt)?;
    let created = match &receipt.request.action {
        MutationAction::Mkdir => platform.create_directory(&context.parent, &context.name),
        MutationAction::Copy { .. } => platform.create_file(&context.parent, &context.name),
    };
    receipt.destination_created = created.destination_created;
    let mut destination = created.file?;
    receipt.destination_created_observation = Some(context.destination_info(&destination)?);
    checkpoint(receipt)?;
    if let MutationAction::Copy { max_bytes, .. } = &receipt.request.action {
        receipt.stage = MutationStage::Copying;
        checkpoint(receipt)?;
        let verification = copying::copy(platform, &context, &mut destination, *max_bytes)?;
        receipt.verification = Some(verification);
    }
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
