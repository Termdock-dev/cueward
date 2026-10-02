use super::*;
use serde::{Deserialize, Serialize};

/// Only a private prepared operation ID travels over the parent lifeline.
#[derive(Debug, Serialize, Deserialize)]
pub struct RelocationWorkerRequest {
    pub operation_id: String,
}
/// Observe a dry run in a read-only deadline-controlled worker.
pub fn run_plan(
    executable: &Path,
    request: &RelocationRequest,
    timeout: u64,
) -> Result<RelocationPlan, FileError> {
    protocol::run_typed(executable, "files-relocation-plan-worker", request, timeout)
}
/// Persist a one-use request and supervise its worker without automatic replay.
pub fn run(
    executable: &Path,
    request: &RelocationRequest,
    timeout: u64,
) -> Result<RelocationReceipt, FileError> {
    protocol::payload(request, timeout)?;
    let prepared = STORE.create(|id, path| RelocationReceipt::new(id, path, request.clone()))?;
    supervision::announce(&prepared.operation_id, "relocation receipt")?;
    let result = supervision::run(
        executable,
        "files-relocation-worker",
        &RelocationWorkerRequest {
            operation_id: prepared.operation_id.clone(),
        },
        timeout,
    )
    .and_then(|receipt| verify_response(&prepared, receipt));
    match result {
        Ok(receipt) => Ok(receipt),
        Err(mut error) => {
            error.message.push_str(
                "; names may have changed; inspect receipt and reobserve before any retry",
            );
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
    prepared: &RelocationReceipt,
    receipt: RelocationReceipt,
) -> Result<RelocationReceipt, FileError> {
    let stored = read_receipt(&prepared.operation_id)?;
    if receipt.operation_id != prepared.operation_id
        || receipt.receipt_path != prepared.receipt_path
        || json(&receipt.request)? != json(&prepared.request)?
        || json(&receipt)? != json(&stored)?
    {
        return Err(FileError::new(
            FileErrorCode::Internal,
            "relocation result differs from prepared/stored evidence",
        ));
    }
    Ok(receipt)
}
/// Query evidence, never resume or undo the operation.
pub fn read_receipt(id: &str) -> Result<RelocationReceipt, FileError> {
    let receipt: RelocationReceipt = STORE.load(id)?;
    if receipt.operation_id != id
        || STORE.directory(id)?.join("receipt.json").to_str() != Some(&receipt.receipt_path)
    {
        return Err(FileError::new(
            FileErrorCode::Internal,
            "relocation receipt identity/path mismatch",
        ));
    }
    Ok(receipt)
}
/// Arm parent lifetime monitoring before any claim or native submission.
pub fn read_supervised_request() -> Result<RelocationWorkerRequest, FileError> {
    supervision::read_request()
}
/// Claim exactly once and persist final outcomes; interrupted evidence is never replayed.
pub fn execute_worker(input: &RelocationWorkerRequest) -> Result<RelocationReceipt, FileError> {
    STORE.claim(&input.operation_id)?;
    let mut receipt = read_receipt(&input.operation_id)?;
    if receipt.stage != RelocationStage::Preflight
        || receipt.mutation_attempted
        || receipt.error.is_some()
    {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "relocation has progressed; replay rejected",
        ));
    }
    let result = policy::NoMaterialization::enter()
        .and_then(|_policy| relocation::execute(&MacFiles, &mut receipt, &mut save));
    if let Err(error) = result {
        receipt.record_error(error);
    }
    save(&receipt)?;
    Ok(receipt)
}
/// Atomically persist bounded relocation evidence.
pub(super) fn save(receipt: &RelocationReceipt) -> Result<(), FileError> {
    STORE.save(&receipt.operation_id, &receipt.receipt_path, receipt)
}
fn json(value: &impl Serialize) -> Result<serde_json::Value, FileError> {
    serde_json::to_value(value).map_err(|e| FileError::new(FileErrorCode::Internal, e.to_string()))
}

/// Allocate one child receipt under an already supervised batch; never dispatch or replay it.
pub(in crate::files) fn execute_child(
    platform: &impl RelocationPlatform,
    request: &RelocationRequest,
    checkpoint: &mut impl FnMut(&RelocationReceipt) -> Result<(), FileError>,
) -> Result<RelocationReceipt, FileError> {
    let mut receipt = STORE.create(|id, path| RelocationReceipt::new(id, path, request.clone()))?;
    let mut persist = |child: &RelocationReceipt| {
        save(child)?;
        checkpoint(child)
    };
    persist(&receipt)?;
    STORE.claim(&receipt.operation_id)?;
    if let Err(error) = relocation::execute(platform, &mut receipt, &mut persist) {
        receipt.record_error(error);
    }
    persist(&receipt)?;
    Ok(receipt)
}
