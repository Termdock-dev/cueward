//! Verified backup, guarded private quarantine, then native trash with actual-result verification.
mod model;
mod context;
pub(super) mod verification;
mod quarantine;
use crate::files::mutation::*;
use crate::files::scope::{Scope, text};
use crate::files::*;
pub use model::*;
use std::fs::File;
use std::path::{Path, PathBuf};

/// Guarded scope removal and native trash of a verified private file; no shell deletion fallback.
pub trait TrashPlatform: MutationPlatform {
    /// Locate an existing private user Trash; never create it or move anything.
    fn trash_directory(&self, source: &FileInfo) -> Result<PathBuf, FileError>;
    /// Require available same-volume private Trash and one ordinary source link.
    fn validate_trash(&self, root: &File, source: &File, trash: &File) -> Result<(), FileError>;
    /// Move relative to held roots exclusively; cannot guarantee source-inode CAS.
    fn stage_source(&self, root: &File, source: &Path, private: &File, name: &Path) -> Publication;
    /// Trash only the verified privately staged ordinary file, returning the native actual URL.
    fn trash_staged(&self, source: &FileInfo) -> NativeTrashResult;
    /// Observe bounded extended attributes without changing file offsets.
    fn trash_attribute_digest(&self, file: &File) -> Result<(String, usize), FileError>;
    /// Verify all original xattrs, recording any narrowly allowed platform-added marker.
    fn verify_trashed_attributes(
        &self,
        file: &File,
        expected: &CopyVerification,
    ) -> Result<TrashAttributes, FileError>;
}
/// Native trash errors cannot establish absence of side effects.
pub struct NativeTrashResult {
    pub result: Result<PathBuf, FileError>,
    pub moved: Option<bool>,
}
/// Validate explicit selection, confirmation, revision guards and finite transfer limits.
pub fn validate_request(request: &TrashRequest) -> Result<(), FileError> {
    super::validate(&request.plan_request())?;
    if !request.confirm
        || request.expected_parent_version.is_empty()
        || !(1..=268435456).contains(&request.max_bytes)
    {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "trash execution requires explicit confirmation, parent revision and max-bytes 1..268435456",
        ));
    }
    Ok(())
}
/// Back up and read back before any namespace removal; persist each pre-write checkpoint.
pub fn execute(
    platform: &impl TrashPlatform,
    receipt: &mut TrashReceipt,
    checkpoint: &mut impl FnMut(&TrashReceipt) -> Result<(), FileError>,
) -> Result<(), FileError> {
    receipt.validate_fresh()?;
    let context = context::Context::prepare(platform, receipt)?;
    let backup = verification::backup(&context, receipt, checkpoint)?;
    context.revalidate(receipt)?;
    verification::check_backup(&context, receipt, &backup)?;
    receipt.stage = TrashStage::BackedUp;
    receipt.backup_verified = true;
    checkpoint(receipt)?;
    quarantine::quarantine(&context, receipt, &backup, checkpoint)?;
    quarantine::native_trash(&context, receipt, &backup, checkpoint)?;
    receipt.stage = TrashStage::Verifying;
    checkpoint(receipt)?;
    verification::finish(&context, receipt, &backup)?;
    receipt.stage = TrashStage::Finished;
    receipt.status = MutationStatus::Completed;
    receipt.completion_verified = true;
    checkpoint(receipt)
}
/// Retain all objects and evidence; classify what is known without retry or cleanup.
pub fn record_error(receipt: &mut TrashReceipt, mut error: FileError) {
    receipt.status = match (
        receipt.source_removed,
        receipt.trash_attempted,
        receipt.moved_to_trash,
    ) {
        (_, true, None) => MutationStatus::Uncertain,
        (Some(true), _, _) => MutationStatus::Incomplete,
        (Some(false), _, _) => MutationStatus::NotStarted,
        (None, _, _) if !receipt.mutation_attempted => MutationStatus::NotStarted,
        _ => MutationStatus::Uncertain,
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
fn json_error(error: serde_json::Error) -> FileError {
    FileError::new(FileErrorCode::Internal, error.to_string())
}
fn bound(receipt: &TrashReceipt) -> Result<(), FileError> {
    // Reserve headroom for final observations and a bounded error in the 256 KiB store.
    if serde_json::to_vec_pretty(receipt)
        .map_err(json_error)?
        .len()
        > 128 * 1024
    {
        return Err(FileError::new(
            FileErrorCode::ScanLimit,
            "trash evidence exceeds 128 KiB safety budget",
        ));
    }
    Ok(())
}
fn observe(platform: &impl FilePlatform, path: &Path) -> Result<FileInfo, FileError> {
    let parent = path
        .parent()
        .ok_or_else(|| changed("missing observation parent"))?;
    let leaf = path
        .file_name()
        .ok_or_else(|| changed("missing observation leaf"))?;
    let scope = Scope::new(platform, parent)?;
    if scope.root != parent {
        return Err(changed(
            "observation parent is not canonical or was retargeted",
        ));
    }
    let info = scope.info(&scope.resolve(Path::new(leaf), false, false)?)?;
    scope.revalidate_root()?;
    Ok(info)
}
