//! Private verified tree construction, one guarded publication, no automatic recovery.
mod context;
mod model;
mod population;
mod verification;
use super::{CopyTreePlan, CopyTreePlatform, CopyTreeRequest};
use crate::files::mutation::*;
use crate::files::scope::{Scope, text};
use crate::files::*;
pub use model::*;
use std::fs::File;
use std::path::{Path, PathBuf};

/// Native attribute observations augment the existing guarded creation/publication hooks.
pub trait TreeExecutionPlatform: CopyTreePlatform + MutationPlatform {
    /// Read bounded xattr digest/value bytes without changing metadata or file offsets.
    fn tree_attribute_digest(&self, file: &File) -> Result<(String, usize), FileError>;
}

/// Validate execution bounds before allocating private evidence.
pub fn validate_request(request: &CopyTreeRequest) -> Result<(), FileError> {
    super::validate(request)?;
    if request.max_entries > MAX_EXECUTION_ENTRIES {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "tree execution accepts at most 64 entries including source",
        ));
    }
    Ok(())
}

/// Execute a fresh tree copy with durable pre-write checkpoints and verified publication.
pub fn execute(
    platform: &impl TreeExecutionPlatform,
    receipt: &mut TreeReceipt,
    checkpoint: &mut impl FnMut(&TreeReceipt) -> Result<(), FileError>,
) -> Result<(), FileError> {
    receipt.validate_fresh()?;
    let context = context::Context::prepare(platform, receipt)?;
    let mut staged = population::prepare(&context, receipt, checkpoint)?;
    population::populate(&context, &mut staged, receipt, checkpoint)?;
    verification::verify_private(&context, &staged, receipt)?;
    context.revalidate(receipt, false)?;
    receipt.active_index = None;
    receipt.staging_verified = true;
    receipt.stage = TreeStage::Staged;
    checkpoint(receipt)?;
    context.revalidate(receipt, false)?;
    receipt.stage = TreeStage::Publishing;
    receipt.mutation_attempted = true;
    checkpoint(receipt)?;
    context
        .revalidate(receipt, false)
        .inspect_err(|_| receipt.destination_created = Some(false))?;
    let publication =
        platform.publish(&context.root, &staged.storage, &receipt.request.destination);
    receipt.destination_created = publication.destination_created;
    publication.result?;
    receipt.stage = TreeStage::Verifying;
    checkpoint(receipt)?;
    verification::finish(&context, &staged, receipt)?;
    receipt.status = MutationStatus::Completed;
    receipt.stage = TreeStage::Finished;
    receipt.completion_verified = true;
    checkpoint(receipt)
}

/// Preserve incomplete or uncertain evidence; never delete or retry anything.
pub fn record_error(receipt: &mut TreeReceipt, mut error: FileError) {
    receipt.status = match receipt.destination_created {
        Some(true) => MutationStatus::Incomplete,
        Some(false) => MutationStatus::NotStarted,
        None if !receipt.mutation_attempted => MutationStatus::NotStarted,
        None => MutationStatus::Uncertain,
    };
    receipt.completion_verified = false;
    if error.message.len() > 1024 {
        let mut end = 1024;
        while !error.message.is_char_boundary(end) {
            end -= 1;
        }
        error.message.truncate(end);
        receipt.error_truncated = true;
    }
    receipt.error = Some(error);
}
fn changed(message: &str) -> FileError {
    FileError::new(FileErrorCode::Changed, message)
}
