use super::*;
use serde::{Deserialize, Serialize};

/// Copy retained verified backup to the recorded original path; never remove Trash or backup.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestoreRequest {
    pub trash_operation_id: String,
    pub root: PathBuf,
    pub expected_parent_version: String,
}
/// Only the publication boundary can create an original-path destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestoreStage {
    Preflight,
    Copying,
    Staged,
    Publishing,
    Verifying,
    Finished,
}
/// Saved restore evidence is separate from immutable original trash evidence.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestoreReceipt {
    pub operation: String,
    pub operation_id: String,
    pub receipt_path: String,
    pub request: RestoreRequest,
    pub status: MutationStatus,
    pub stage: RestoreStage,
    pub trash_receipt_sha256: Option<String>,
    pub destination: Option<PathBuf>,
    pub original_source: Option<FileInfo>,
    pub root_before: Option<FileInfo>,
    pub parent_before: Option<FileInfo>,
    pub backup_before: Option<FileInfo>,
    pub staging_path: Option<String>,
    pub staging_after: Option<FileInfo>,
    pub verification: Option<CopyVerification>,
    pub staging_verified: bool,
    pub mutation_attempted: bool,
    pub destination_created: Option<bool>,
    pub root_after: Option<FileInfo>,
    pub parent_after: Option<FileInfo>,
    pub destination_after: Option<FileInfo>,
    pub backup_after: Option<FileInfo>,
    pub completion_verified: bool,
    pub error: Option<FileError>,
    pub error_truncated: bool,
    pub preflight_observations_omitted: bool,
    pub recovery_method: String,
    pub trash_item_touched: bool,
    pub backup_removed: bool,
    pub provider_coordination: String,
}
impl RestoreReceipt {
    /// Prepare a one-use restore request before worker dispatch.
    pub fn new(id: String, path: String, request: RestoreRequest) -> Self {
        Self {
            operation: "trash_restore".into(),
            operation_id: id,
            receipt_path: path,
            request,
            status: MutationStatus::Uncertain,
            stage: RestoreStage::Preflight,
            trash_receipt_sha256: None,
            destination: None,
            original_source: None,
            root_before: None,
            parent_before: None,
            backup_before: None,
            staging_path: None,
            staging_after: None,
            verification: None,
            staging_verified: false,
            mutation_attempted: false,
            destination_created: None,
            root_after: None,
            parent_after: None,
            destination_after: None,
            backup_after: None,
            completion_verified: false,
            error: None,
            error_truncated: false,
            preflight_observations_omitted: false,
            recovery_method: "verified_backup_copy".into(),
            trash_item_touched: false,
            backup_removed: false,
            provider_coordination: "filesystem_only_provider_state_unknown".into(),
        }
    }
    /// Reject progressed records; the caller also claims the UUID exactly once.
    pub fn validate_fresh(&self) -> Result<(), FileError> {
        validate_request(&self.request)?;
        uuid(&self.operation_id)?;
        let fresh = Self::new(
            self.operation_id.clone(),
            self.receipt_path.clone(),
            self.request.clone(),
        );
        if !Path::new(&self.receipt_path).is_absolute() || value(self)? != value(&fresh)? {
            return Err(invalid("restore receipt is not fresh; replay rejected"));
        }
        Ok(())
    }
}
