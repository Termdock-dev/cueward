//! Supervised one-use backup restoration in the existing guarded file-operation store.
use crate::files::{MacFiles, mutation::supervision, policy, protocol, store::Store};
use cueward_core::files::mutation::MutationStatus;
use cueward_core::files::trash::execution::TrashPlatform;
use cueward_core::files::trash::restore;
use cueward_core::files::trash::restore::RestoreRequest;
pub use cueward_core::files::trash::restore::RestoreRequest as Request;
pub use cueward_core::files::trash::restore::{RestoreReceipt, RestoreStage};
use cueward_core::files::*;
use serde::{Deserialize, Serialize};
use std::path::Path;
const STORE: Store = Store("files");

/// The framed transport names only a private prepared operation.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestoreWorkerRequest {
    pub operation_id: String,
}

/// Run a fresh explicit request under one parent lifetime and worker deadline.
pub fn run(
    executable: &Path,
    request: &RestoreRequest,
    timeout: u64,
) -> Result<RestoreReceipt, FileError> {
    protocol::payload(request, timeout)?;
    restore::validate_request(request)?;
    let prepared = STORE.create(|id, path| RestoreReceipt::new(id, path, request.clone()))?;
    supervision::announce(&prepared.operation_id, "trash restore-receipt")?;
    let result = supervision::run(
        executable,
        "files-trash-restore-worker",
        &RestoreWorkerRequest {
            operation_id: prepared.operation_id.clone(),
        },
        timeout,
    )
    .and_then(|receipt| verify_response(&prepared, receipt));
    match result {
        Ok(receipt) => Ok(receipt),
        Err(mut error) => {
            error.message.push_str("; inspect the receipt, restore staging and original destination; never automatically retry or delete");
            let mut receipt = read_receipt(&prepared.operation_id).unwrap_or(prepared);
            restore::record_error(&mut receipt, error);
            receipt.status = MutationStatus::Uncertain;
            save(&receipt)?;
            Ok(receipt)
        }
    }
}
fn verify_response(
    prepared: &RestoreReceipt,
    receipt: RestoreReceipt,
) -> Result<RestoreReceipt, FileError> {
    let stored = read_receipt(&prepared.operation_id)?;
    if receipt.operation_id != prepared.operation_id
        || receipt.receipt_path != prepared.receipt_path
        || json(&receipt.request)? != json(&prepared.request)?
        || json(&receipt)? != json(&stored)?
    {
        return Err(FileError::new(
            FileErrorCode::Internal,
            "restore worker response differs from prepared/stored evidence",
        ));
    }
    Ok(receipt)
}
/// Load typed saved evidence only, without resuming, reconciling or rolling back.
pub fn read_receipt(id: &str) -> Result<RestoreReceipt, FileError> {
    let receipt: RestoreReceipt = STORE.load(id)?;
    if receipt.operation != "trash_restore"
        || receipt.operation_id != id
        || STORE.directory(id)?.join("receipt.json").to_str() != Some(&receipt.receipt_path)
    {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "restore receipt schema/identity/path mismatch",
        ));
    }
    Ok(receipt)
}
/// Arm parent-lifetime supervision before any restore worker operations.
pub fn read_supervised_request() -> Result<RestoreWorkerRequest, FileError> {
    supervision::read_request()
}
/// Claim once and persist all known failures without retry or cleanup.
pub fn execute_worker(input: &RestoreWorkerRequest) -> Result<RestoreReceipt, FileError> {
    execute_prepared(&MacFiles, input)
}
fn execute_prepared(
    platform: &impl TrashPlatform,
    input: &RestoreWorkerRequest,
) -> Result<RestoreReceipt, FileError> {
    STORE.claim(&input.operation_id)?;
    let mut receipt = read_receipt(&input.operation_id)?;
    receipt.validate_fresh()?;
    let result = policy::NoMaterialization::enter().and_then(|_policy| {
        let original = super::execution::read_receipt(&receipt.request.trash_operation_id)?;
        let expected = restore::fingerprint(&original)?;
        restore::execute(platform, &mut receipt, &original, &mut |r| {
            let current = super::execution::read_receipt(&r.request.trash_operation_id)?;
            if restore::fingerprint(&current)? != expected {
                return Err(FileError::new(
                    FileErrorCode::Changed,
                    "original trash receipt changed during restore",
                ));
            }
            save(r)
        })
    });
    if let Err(error) = result {
        restore::record_error(&mut receipt, error);
    }
    save(&receipt)?;
    Ok(receipt)
}
fn save(receipt: &RestoreReceipt) -> Result<(), FileError> {
    STORE.save(&receipt.operation_id, &receipt.receipt_path, receipt)
}
fn json(value: &impl Serialize) -> Result<serde_json::Value, FileError> {
    serde_json::to_value(value).map_err(|e| FileError::new(FileErrorCode::Internal, e.to_string()))
}

#[cfg(test)]
#[path = "restore_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "restore_safety_tests.rs"]
mod safety_tests;
#[cfg(test)]
#[path = "restore_lifetime_tests.rs"]
mod lifetime_tests;
