//! Supervised no-overwrite mutations with persistent uncertain/partial receipts.
mod attributes;
mod journal;
mod native;
mod publication;
mod supervision;
#[cfg(test)]
mod tests;

use super::{MacFiles, policy, protocol};
use cueward_core::files::mutation::record_error;
pub use cueward_core::files::mutation::{
    CopyVerification, Creation, MutationAction, MutationPlatform, MutationReceipt, MutationRequest,
    MutationStage, MutationStatus, Publication, Staging,
};
use cueward_core::files::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Internal worker input refers only to a newly prepared private operation receipt.
#[derive(Debug, Serialize, Deserialize)]
pub struct MutationWorkerRequest {
    pub operation_id: String,
}

/// Run once; failed transport is uncertain, never proof that destination creation was cancelled.
pub fn run(
    executable: &Path,
    request: &MutationRequest,
    timeout_ms: u64,
) -> Result<MutationReceipt, FileError> {
    if !(1..=30000).contains(&timeout_ms) {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "timeout-ms must be 1..30000",
        ));
    }
    // Validate JSON/size before preparing a durable request or starting a worker.
    let payload = serde_json::to_vec(request)
        .map_err(|e| FileError::new(FileErrorCode::InvalidOptions, e.to_string()))?;
    if payload.len() > protocol::MAX_REQUEST_BYTES {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "request exceeds 16 KiB",
        ));
    }
    let prepared = journal::create(request)?;
    let input = MutationWorkerRequest {
        operation_id: prepared.operation_id.clone(),
    };
    supervision::announce(&prepared.operation_id)?;
    let result = supervision::run(executable, &input, timeout_ms)
        .and_then(|receipt| verify_response(&prepared, receipt));
    match result {
        Ok(receipt) => Ok(receipt),
        Err(mut error) => {
            error.message.push_str("; destination may exist or be partial; inspect the receipt and paths, do not automatically retry");
            let mut receipt = journal::load(&prepared.operation_id).unwrap_or(prepared);
            receipt.status = MutationStatus::Uncertain;
            receipt.completion_verified = false;
            receipt.error = Some(error);
            journal::save(&receipt)?;
            Ok(receipt)
        }
    }
}

fn verify_response(
    prepared: &MutationReceipt,
    receipt: MutationReceipt,
) -> Result<MutationReceipt, FileError> {
    let stored = journal::load(&prepared.operation_id)?;
    let json = |value| {
        serde_json::to_value(value)
            .map_err(|e| FileError::new(FileErrorCode::Internal, e.to_string()))
    };
    if receipt.operation_id != prepared.operation_id
        || receipt.receipt_path != prepared.receipt_path
        || json(&receipt.request)? != json(&prepared.request)?
        || serde_json::to_value(&receipt)
            .map_err(|e| FileError::new(FileErrorCode::Internal, e.to_string()))?
            != serde_json::to_value(stored)
                .map_err(|e| FileError::new(FileErrorCode::Internal, e.to_string()))?
    {
        return Err(FileError::new(
            FileErrorCode::Internal,
            "worker result differs from its durable operation receipt",
        ));
    }
    Ok(receipt)
}

/// Read saved evidence only; this does not resume, retry or reconcile current filesystem state.
pub fn read_receipt(operation_id: &str) -> Result<MutationReceipt, FileError> {
    journal::load(operation_id)
}

/// Claim a private operation exactly once and persist each pre-write checkpoint.
pub fn execute_worker(input: &MutationWorkerRequest) -> Result<MutationReceipt, FileError> {
    journal::claim(&input.operation_id)?;
    let mut receipt = journal::load(&input.operation_id)?;
    if receipt.stage != MutationStage::Preflight
        || receipt.mutation_attempted
        || receipt.error.is_some()
    {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "operation has already progressed; replay is not supported",
        ));
    }
    let result = policy::NoMaterialization::enter().and_then(|_policy| {
        cueward_core::files::mutation::execute(&MacFiles, &mut receipt, &mut journal::save)
    });
    if let Err(error) = result {
        record_error(&mut receipt, error);
    }
    journal::save(&receipt)?;
    Ok(receipt)
}

/// Read the framed worker request and arm fail-closed parent-lifetime monitoring before writes.
pub fn read_supervised_request() -> Result<MutationWorkerRequest, FileError> {
    supervision::read_request()
}
