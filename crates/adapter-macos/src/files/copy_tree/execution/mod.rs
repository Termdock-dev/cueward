//! Supervised one-use tree copies in the existing guarded file-operation store.
use crate::files::{MacFiles, mutation::supervision, policy, protocol, store::Store};
use cueward_core::files::copy_tree::CopyTreeRequest;
use cueward_core::files::copy_tree::execution::{self, TreeExecutionPlatform};
pub use cueward_core::files::copy_tree::execution::{TreeReceipt, TreeStage};
use cueward_core::files::mutation::MutationStatus;
use cueward_core::files::*;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::Path;
const STORE: Store = Store("files");

impl TreeExecutionPlatform for MacFiles {
    fn tree_attribute_digest(&self, file: &File) -> Result<(String, usize), FileError> {
        crate::files::mutation::attributes::digest(file)
    }
}
/// The framed transport names only a private prepared operation.
#[derive(Debug, Serialize, Deserialize)]
pub struct TreeWorkerRequest {
    pub operation_id: String,
}

/// Run a fresh explicit request under one parent lifetime and worker deadline.
pub fn run(
    executable: &Path,
    request: &CopyTreeRequest,
    timeout: u64,
) -> Result<TreeReceipt, FileError> {
    protocol::payload(request, timeout)?;
    execution::validate_request(request)?;
    let prepared = STORE.create(|id, path| TreeReceipt::new(id, path, request.clone()))?;
    supervision::announce(&prepared.operation_id, "copy-tree receipt")?;
    let result = supervision::run(
        executable,
        "files-copy-tree-worker",
        &TreeWorkerRequest {
            operation_id: prepared.operation_id.clone(),
        },
        timeout,
    )
    .and_then(|receipt| verify_response(&prepared, receipt));
    match result {
        Ok(receipt) => Ok(receipt),
        Err(mut error) => {
            error.message.push_str("; inspect the receipt, private staging and destination; never automatically retry or delete");
            let mut receipt = read_receipt(&prepared.operation_id).unwrap_or(prepared);
            execution::record_error(&mut receipt, error);
            receipt.status = MutationStatus::Uncertain;
            save(&receipt)?;
            Ok(receipt)
        }
    }
}
fn verify_response(prepared: &TreeReceipt, receipt: TreeReceipt) -> Result<TreeReceipt, FileError> {
    let stored = read_receipt(&prepared.operation_id)?;
    if receipt.operation_id != prepared.operation_id
        || receipt.receipt_path != prepared.receipt_path
        || json(&receipt.request)? != json(&prepared.request)?
        || json(&receipt)? != json(&stored)?
    {
        return Err(FileError::new(
            FileErrorCode::Internal,
            "tree worker response differs from prepared/stored evidence",
        ));
    }
    Ok(receipt)
}
/// Load typed saved evidence only, without resuming, reconciling or rolling back.
pub fn read_receipt(id: &str) -> Result<TreeReceipt, FileError> {
    let receipt: TreeReceipt = STORE.load(id)?;
    if receipt.operation != "copy_tree"
        || receipt.operation_id != id
        || STORE.directory(id)?.join("receipt.json").to_str() != Some(&receipt.receipt_path)
    {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "tree receipt schema/identity/path mismatch",
        ));
    }
    Ok(receipt)
}
/// Arm parent-lifetime supervision before any tree worker operations.
pub fn read_supervised_request() -> Result<TreeWorkerRequest, FileError> {
    supervision::read_request()
}
/// Claim once and persist all known failures without retry or cleanup.
pub fn execute_worker(input: &TreeWorkerRequest) -> Result<TreeReceipt, FileError> {
    execute_prepared(&MacFiles, input)
}
fn execute_prepared(
    platform: &impl TreeExecutionPlatform,
    input: &TreeWorkerRequest,
) -> Result<TreeReceipt, FileError> {
    STORE.claim(&input.operation_id)?;
    let mut receipt = read_receipt(&input.operation_id)?;
    receipt.validate_fresh()?;
    let result = policy::NoMaterialization::enter()
        .and_then(|_policy| execution::execute(platform, &mut receipt, &mut save));
    if let Err(error) = result {
        execution::record_error(&mut receipt, error);
    }
    save(&receipt)?;
    Ok(receipt)
}
fn save(receipt: &TreeReceipt) -> Result<(), FileError> {
    STORE.save(&receipt.operation_id, &receipt.receipt_path, receipt)
}
fn json(value: &impl Serialize) -> Result<serde_json::Value, FileError> {
    serde_json::to_value(value).map_err(|e| FileError::new(FileErrorCode::Internal, e.to_string()))
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod safety_tests;

#[cfg(test)]
mod package_tests;
