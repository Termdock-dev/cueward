#[path = "native.rs"]
mod native;
// Supervised one-use trash operations in the existing guarded file-operation store.
use crate::files::{MacFiles, mutation::supervision, policy, protocol, store::Store};
use cueward_core::files::mutation::MutationStatus;
use cueward_core::files::trash::execution::TrashRequest;
pub use cueward_core::files::trash::execution::TrashRequest as Request;
use cueward_core::files::trash::execution::{self, TrashPlatform};
pub use cueward_core::files::trash::execution::{TrashReceipt, TrashStage};
use cueward_core::files::*;
use serde::{Deserialize, Serialize};
use std::path::Path;
const STORE: Store = Store("files");

/// The framed transport names only a private prepared operation.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrashWorkerRequest {
    pub operation_id: String,
}

/// Run a fresh explicit request under one parent lifetime and worker deadline.
pub fn run(
    executable: &Path,
    request: &TrashRequest,
    timeout: u64,
) -> Result<TrashReceipt, FileError> {
    protocol::payload(request, timeout)?;
    execution::validate_request(request)?;
    let prepared = STORE.create(|id, path| TrashReceipt::new(id, path, request.clone()))?;
    supervision::announce(&prepared.operation_id, "trash receipt")?;
    let result = supervision::run(
        executable,
        "files-trash-worker",
        &TrashWorkerRequest {
            operation_id: prepared.operation_id.clone(),
        },
        timeout,
    )
    .and_then(|receipt| verify_response(&prepared, receipt));
    match result {
        Ok(receipt) => Ok(receipt),
        Err(mut error) => {
            error.message.push_str("; inspect the receipt, private backup and Trash entry; never automatically retry or delete");
            let mut receipt = read_receipt(&prepared.operation_id).unwrap_or(prepared);
            execution::record_error(&mut receipt, error);
            receipt.status = MutationStatus::Uncertain;
            save(&receipt)?;
            Ok(receipt)
        }
    }
}
fn verify_response(
    prepared: &TrashReceipt,
    receipt: TrashReceipt,
) -> Result<TrashReceipt, FileError> {
    let stored = read_receipt(&prepared.operation_id)?;
    if receipt.operation_id != prepared.operation_id
        || receipt.receipt_path != prepared.receipt_path
        || json(&receipt.request)? != json(&prepared.request)?
        || json(&receipt)? != json(&stored)?
    {
        return Err(FileError::new(
            FileErrorCode::Internal,
            "trash worker response differs from prepared/stored evidence",
        ));
    }
    Ok(receipt)
}
/// Load typed saved evidence only, without resuming, reconciling or rolling back.
pub fn read_receipt(id: &str) -> Result<TrashReceipt, FileError> {
    let receipt: TrashReceipt = STORE.load(id)?;
    if receipt.operation != "trash"
        || receipt.operation_id != id
        || STORE.directory(id)?.join("receipt.json").to_str() != Some(&receipt.receipt_path)
    {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "trash receipt schema/identity/path mismatch",
        ));
    }
    Ok(receipt)
}
/// Arm parent-lifetime supervision before any trash worker operations.
pub fn read_supervised_request() -> Result<TrashWorkerRequest, FileError> {
    supervision::read_request()
}
/// Claim once and persist all known failures without retry or cleanup.
pub fn execute_worker(input: &TrashWorkerRequest) -> Result<TrashReceipt, FileError> {
    execute_prepared(&MacFiles, input)
}
fn execute_prepared(
    platform: &impl TrashPlatform,
    input: &TrashWorkerRequest,
) -> Result<TrashReceipt, FileError> {
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
fn save(receipt: &TrashReceipt) -> Result<(), FileError> {
    STORE.save(&receipt.operation_id, &receipt.receipt_path, receipt)
}
fn json(value: &impl Serialize) -> Result<serde_json::Value, FileError> {
    serde_json::to_value(value).map_err(|e| FileError::new(FileErrorCode::Internal, e.to_string()))
}

#[cfg(test)]
#[path = "tests.rs"]
pub(super) mod tests;
#[cfg(test)]
#[path = "safety_tests.rs"]
mod safety_tests;
#[cfg(test)]
#[path = "lifetime_tests.rs"]
mod lifetime_tests;
