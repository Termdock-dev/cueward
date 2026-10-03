//! Non-destructive backup recovery to the recorded original path; never overwrite or clean up.
mod model;
mod context;
mod copying;
mod evidence;
use super::execution::verification::verify_file;
use super::execution::{TrashPlatform, TrashReceipt, TrashStage};
use crate::files::mutation::*;
use crate::files::scope::{Scope, text};
use crate::files::*;
pub use model::*;
use serde::Serialize;
use std::fs::File;
use std::path::{Path, PathBuf};

/// Validate bounded explicit restore selection before allocating a new receipt.
pub fn validate_request(request: &RestoreRequest) -> Result<(), FileError> {
    uuid(&request.trash_operation_id)?;
    if !request.root.is_absolute() || request.expected_parent_version.is_empty() {
        return Err(invalid(
            "restore requires absolute root and fresh original-parent revision",
        ));
    }
    text(&request.root)?;
    Ok(())
}
/// Build/read back a fresh copy from saved backup, then publish once without overwrite.
pub fn execute(
    platform: &impl TrashPlatform,
    receipt: &mut RestoreReceipt,
    original: &TrashReceipt,
    checkpoint: &mut impl FnMut(&RestoreReceipt) -> Result<(), FileError>,
) -> Result<(), FileError> {
    receipt.validate_fresh()?;
    let mut context = context::Context::prepare(platform, receipt, original)?;
    checkpoint(receipt)?;
    let (storage, candidate) = copying::prepare(&mut context, receipt, checkpoint)?;
    context.revalidate(receipt, false)?;
    copying::verify_candidate(&context, receipt, &candidate, &storage.path)?;
    receipt.stage = RestoreStage::Staged;
    receipt.staging_verified = true;
    checkpoint(receipt)?;
    receipt.stage = RestoreStage::Publishing;
    receipt.mutation_attempted = true;
    checkpoint(receipt)?;
    context
        .revalidate(receipt, false)
        .inspect_err(|_| receipt.destination_created = Some(false))?;
    copying::verify_candidate(&context, receipt, &candidate, &storage.path)
        .inspect_err(|_| receipt.destination_created = Some(false))?;
    let publication = platform.publish(&context.root, &storage, &context.destination);
    receipt.destination_created = publication.destination_created;
    publication.result?;
    receipt.stage = RestoreStage::Verifying;
    checkpoint(receipt)?;
    copying::finish(&context, receipt, &candidate)?;
    receipt.stage = RestoreStage::Finished;
    receipt.status = MutationStatus::Completed;
    receipt.completion_verified = true;
    bound(receipt, 128 * 1024)?;
    checkpoint(receipt)
}
/// Preserve staging and destination on every failure; never retry, rollback or remove originals.
pub fn record_error(receipt: &mut RestoreReceipt, mut error: FileError) {
    receipt.status = match receipt.destination_created {
        Some(true) => MutationStatus::Incomplete,
        Some(false) => MutationStatus::NotStarted,
        None if !receipt.mutation_attempted => MutationStatus::NotStarted,
        None => MutationStatus::Uncertain,
    };
    receipt.completion_verified = false;
    if error.message.len() > 1024 {
        let mut n = 1024;
        while !error.message.is_char_boundary(n) {
            n -= 1;
        }
        error.message.truncate(n);
        receipt.error_truncated = true;
    }
    receipt.error = Some(error);
}
/// Hash original typed evidence for consistency checks, not as an authenticity signature.
pub fn fingerprint(original: &TrashReceipt) -> Result<String, FileError> {
    use sha2::{Digest, Sha256};
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec_pretty(original).map_err(json_error)?)
    ))
}
fn uuid(id: &str) -> Result<(), FileError> {
    if id.len() != 36
        || id.bytes().enumerate().any(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b != b'-'
            } else {
                !b.is_ascii_digit() && !(b'a'..=b'f').contains(&b)
            }
        })
    {
        return Err(invalid("operation-id must be a canonical UUID"));
    }
    Ok(())
}
fn invalid(message: &str) -> FileError {
    FileError::new(FileErrorCode::InvalidOptions, message)
}
fn changed(message: &str) -> FileError {
    FileError::new(FileErrorCode::Changed, message)
}
fn json_error(error: serde_json::Error) -> FileError {
    FileError::new(FileErrorCode::Internal, error.to_string())
}
fn value(value: &impl Serialize) -> Result<serde_json::Value, FileError> {
    serde_json::to_value(value).map_err(json_error)
}
fn bound(receipt: &RestoreReceipt, limit: usize) -> Result<(), FileError> {
    if serde_json::to_vec_pretty(receipt)
        .map_err(json_error)?
        .len()
        > limit
    {
        return Err(FileError::new(
            FileErrorCode::ScanLimit,
            "restore evidence exceeds safety budget",
        ));
    }
    Ok(())
}
fn observe(platform: &impl FilePlatform, path: &Path) -> Result<FileInfo, FileError> {
    let parent = path
        .parent()
        .ok_or_else(|| changed("missing observation parent"))?;
    let scope = Scope::new(platform, parent)?;
    if scope.root != parent {
        return Err(changed("observation parent retargeted"));
    }
    let info = scope.info(&scope.resolve(
        Path::new(path.file_name().ok_or_else(|| changed("missing leaf"))?),
        false,
        true,
    )?)?;
    scope.revalidate_root()?;
    Ok(info)
}
