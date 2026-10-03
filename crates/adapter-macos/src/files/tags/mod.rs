//! Tag observations, atomic initial tagging and no-ops with parent-bound receipts.
mod codec;
mod context;
mod model;
mod native;
mod operation;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod safety_tests;
use super::{mutation::supervision, policy, protocol, store::Store};
use cueward_core::files::tags::{FileTag, TagSnapshot, edit_tags};
use cueward_core::files::*;
pub use model::*;
use std::fs::File;
use std::path::Path;
const STORE: Store = Store("files-tags");

/// Read the exact tag revision without materializing data or following links.
pub fn read(root: &Path, path: &Path, expected: Option<String>) -> Result<TagSnapshot, FileError> {
    let _policy = policy::NoMaterialization::enter()?;
    Ok(context::Context::prepare(root, path, expected)?.before)
}
/// Read-only queries retain normal deadline-controlled transport.
pub fn run_read(
    executable: &Path,
    root: &Path,
    path: &Path,
    timeout: u64,
) -> Result<TagSnapshot, FileError> {
    protocol::run_typed(executable, "files-tags-read-worker", &(root, path), timeout)
}
/// Prepare a private one-use edit and keep the parent lifeline open until response.
pub fn run(
    executable: &Path,
    request: &TagsRequest,
    timeout: u64,
) -> Result<TagsReceipt, FileError> {
    protocol::payload(request, timeout)?;
    validate(request)?;
    let prepared = STORE.create(|id, path| TagsReceipt::new(id, path, request.clone()))?;
    supervision::announce(&prepared.operation_id, "tags receipt")?;
    let result = supervision::run(
        executable,
        "files-tags-worker",
        &TagsWorkerRequest {
            operation_id: prepared.operation_id.clone(),
        },
        timeout,
    )
    .and_then(|receipt: TagsReceipt| {
        protocol::verify_receipt(
            &prepared,
            receipt,
            &read_receipt(&prepared.operation_id)?,
            "tag worker result differs from prepared/stored evidence",
        )
    });
    match result {
        Ok(receipt) => Ok(receipt),
        Err(mut error) => {
            error.message.push_str(
                "; tags may have changed; inspect receipt and reobserve before any retry",
            );
            let mut receipt = read_receipt(&prepared.operation_id).unwrap_or(prepared);
            receipt.status = TagsStatus::Uncertain;
            receipt.completion_verified = false;
            receipt.error = Some(error);
            save(&receipt)?;
            Ok(receipt)
        }
    }
}

/// Read saved evidence only, without resuming or restoring original tags.
pub fn read_receipt(id: &str) -> Result<TagsReceipt, FileError> {
    let receipt: TagsReceipt = STORE.load(id)?;
    if receipt.operation_id != id
        || STORE.directory(id)?.join("receipt.json").to_str() != Some(&receipt.receipt_path)
    {
        return Err(FileError::new(
            FileErrorCode::Internal,
            "tag receipt identity/path mismatch",
        ));
    }
    Ok(receipt)
}
/// Arm lifetime monitoring before claiming or mutating the selected object.
pub fn read_supervised_request() -> Result<TagsWorkerRequest, FileError> {
    supervision::read_request()
}
/// Claim exactly once and finalize local errors without automatic retry or rollback.
pub fn execute_worker(input: &TagsWorkerRequest) -> Result<TagsReceipt, FileError> {
    STORE.claim(&input.operation_id)?;
    let mut receipt = read_receipt(&input.operation_id)?;
    if receipt.stage != TagsStage::Preflight
        || receipt.mutation_attempted
        || receipt.error.is_some()
    {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "tag operation has progressed; replay rejected",
        ));
    }
    let result = policy::NoMaterialization::enter()
        .and_then(|_policy| operation::execute(&mut receipt, &mut save));
    if let Err(error) = result {
        record_error(&mut receipt, error);
    }
    save(&receipt)?;
    Ok(receipt)
}
fn validate(request: &TagsRequest) -> Result<(), FileError> {
    if request.expected_version.is_empty() || request.expected_tags_version.is_empty() {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "tag edits require expected-version and expected-tags-version",
        ));
    }
    edit_tags(&[], &request.edit)?;
    Ok(())
}
fn save(receipt: &TagsReceipt) -> Result<(), FileError> {
    STORE.save(&receipt.operation_id, &receipt.receipt_path, receipt)
}
fn changed(message: &str) -> FileError {
    FileError::new(FileErrorCode::Changed, message)
}
fn record_error(receipt: &mut TagsReceipt, error: FileError) {
    receipt.status = match receipt.changed_by_operation {
        Some(true) => TagsStatus::Incomplete,
        Some(false) => TagsStatus::NotStarted,
        None if !receipt.mutation_attempted => TagsStatus::NotStarted,
        None => TagsStatus::Uncertain,
    };
    receipt.completion_verified = false;
    receipt.error = Some(error);
}
