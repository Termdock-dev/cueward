use super::super::store::Store;
use super::*;
const STORE: Store = Store("files");

/// Validate a private operation directory in the existing files receipt namespace.
pub(super) fn directory(id: &str) -> Result<PathBuf, FileError> {
    STORE.directory(id)
}
/// Prepare the initial mkdir/copy record without changing its saved schema.
pub(super) fn create(request: &MutationRequest) -> Result<MutationReceipt, FileError> {
    STORE.create(|id, path| MutationReceipt::new(id, request.clone(), path))
}
/// Persist one bounded checkpoint without deleting staged payloads.
pub(super) fn save(receipt: &MutationReceipt) -> Result<(), FileError> {
    STORE.save(&receipt.operation_id, &receipt.receipt_path, receipt)
}
/// Read evidence and verify the original operation identity/path.
pub(super) fn load(id: &str) -> Result<MutationReceipt, FileError> {
    let receipt: MutationReceipt = STORE.load(id)?;
    if receipt.operation_id != id
        || STORE.directory(id)?.join("receipt.json").to_str() != Some(&receipt.receipt_path)
    {
        return Err(FileError::new(
            FileErrorCode::Internal,
            "receipt identity/path mismatch",
        ));
    }
    Ok(receipt)
}
/// Claim the operation exactly once without replacing previous evidence.
pub(super) fn claim(id: &str) -> Result<(), FileError> {
    STORE.claim(id)
}
