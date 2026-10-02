//! One supervised fail-stop batch, with separate native receipts for each allocated item.
use super::{MacFiles, mutation::supervision, policy, protocol, relocation, store::Store};
use cueward_core::files::relocation::RelocationStatus;
use cueward_core::files::relocation::batch::execution;
pub use cueward_core::files::relocation::batch::execution::BatchExecutionReceipt;
use cueward_core::files::relocation::batch::{BatchRenamePlatform, BatchRenameRequest};
use cueward_core::files::{FileError, FileErrorCode, FileInfo};
use serde::{Deserialize, Serialize};
use std::path::Path;
const STORE: Store = Store("files-batch-rename");

/// Only the private one-use ID crosses the supervised worker lifeline.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchExecutionWorkerRequest {
    pub operation_id: String,
}

/// Persist the explicit request and publish lookup before starting any batch writes.
pub fn run(
    executable: &Path,
    request: &BatchRenameRequest,
    timeout: u64,
) -> Result<BatchExecutionReceipt, FileError> {
    protocol::payload(request, timeout)?;
    if !(1..=64).contains(&request.entries.len()) {
        return Err(invalid("batch requires 1..64 entries"));
    }
    let prepared =
        STORE.create(|id, path| BatchExecutionReceipt::new(id, path, request.clone()))?;
    supervision::announce(&prepared.operation_id, "rename-batch receipt")?;
    let result = supervision::run(
        executable,
        "files-batch-rename-worker",
        &BatchExecutionWorkerRequest {
            operation_id: prepared.operation_id.clone(),
        },
        timeout,
    )
    .and_then(|receipt| verify_response(&prepared, receipt));
    match result {
        Ok(receipt) => Ok(receipt),
        Err(error) => {
            let mut receipt = read_receipt(&prepared.operation_id).unwrap_or(prepared);
            receipt.status = RelocationStatus::Uncertain;
            receipt.completion_verified = false;
            receipt.error = Some(error);
            save(&receipt)?;
            Ok(receipt)
        }
    }
}
fn verify_response(
    prepared: &BatchExecutionReceipt,
    receipt: BatchExecutionReceipt,
) -> Result<BatchExecutionReceipt, FileError> {
    let stored = read_receipt(&prepared.operation_id)?;
    if receipt.operation_id != prepared.operation_id
        || receipt.receipt_path != prepared.receipt_path
        || json(&receipt.request)? != json(&prepared.request)?
        || json(&receipt)? != json(&stored)?
    {
        return Err(FileError::new(
            FileErrorCode::Internal,
            "batch response differs from prepared or saved evidence",
        ));
    }
    Ok(receipt)
}
/// Load aggregate progress only; individual evidence uses files relocation receipt.
pub fn read_receipt(id: &str) -> Result<BatchExecutionReceipt, FileError> {
    let receipt: BatchExecutionReceipt = STORE.load(id)?;
    if receipt.operation_id != id
        || STORE.directory(id)?.join("receipt.json").to_str() != Some(&receipt.receipt_path)
    {
        return Err(FileError::new(
            FileErrorCode::Internal,
            "batch receipt identity/path mismatch",
        ));
    }
    Ok(receipt)
}
/// Reject parent death before claiming an operation or preparing native writes.
pub fn read_supervised_request() -> Result<BatchExecutionWorkerRequest, FileError> {
    supervision::read_request()
}

/// Claim once, checkpoint partial progress and stop at the first failure without recovery.
pub fn execute_worker(
    input: &BatchExecutionWorkerRequest,
) -> Result<BatchExecutionReceipt, FileError> {
    execute_prepared(&MacFiles, input)
}
fn execute_prepared(
    platform: &impl BatchRenamePlatform,
    input: &BatchExecutionWorkerRequest,
) -> Result<BatchExecutionReceipt, FileError> {
    STORE.claim(&input.operation_id)?;
    let mut receipt = read_receipt(&input.operation_id)?;
    protocol::payload(&receipt.request, 10000)?;
    if receipt.started || receipt.error.is_some() {
        return Err(invalid("batch has progressed; replay rejected"));
    }
    let result = policy::NoMaterialization::enter().and_then(|_policy| {
        execution::execute(
            platform,
            &mut receipt,
            &mut save,
            &mut |index, request, root, batch| invoke(platform, index, request, root, batch),
        )
    });
    if let Err(error) = result {
        receipt.record_error(error);
    }
    save(&receipt)?;
    Ok(receipt)
}
fn invoke(
    platform: &impl BatchRenamePlatform,
    index: usize,
    request: &relocation::RelocationRequest,
    root: &FileInfo,
    batch: &mut BatchExecutionReceipt,
) -> Result<relocation::RelocationReceipt, FileError> {
    relocation::execute_child(platform, request, &mut |child| {
        // Context has held this root before preparing the native call. Reject a
        // replacement between aggregate replanning and the child's preparation.
        if child.error.is_none()
            && let Some(before) = &child.before
            && (before.root.path != root.path
                || before.root.identity != root.identity
                || before.root.version != root.version)
        {
            return Err(FileError::new(
                FileErrorCode::Changed,
                "child root differs from batch anchor",
            ));
        }
        batch.observe_child(index, child);
        save(batch)
    })
}
fn save(receipt: &BatchExecutionReceipt) -> Result<(), FileError> {
    STORE.save(&receipt.operation_id, &receipt.receipt_path, receipt)
}
fn invalid(message: &str) -> FileError {
    FileError::new(FileErrorCode::InvalidOptions, message)
}
fn json(value: &impl Serialize) -> Result<serde_json::Value, FileError> {
    serde_json::to_value(value).map_err(|e| FileError::new(FileErrorCode::Internal, e.to_string()))
}

#[cfg(test)]
#[path = "batch_execution_tests.rs"]
mod tests;
