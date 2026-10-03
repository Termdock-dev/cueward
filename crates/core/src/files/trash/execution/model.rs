use super::*;
use serde::{Deserialize, Serialize};

/// Explicit confirmation is separate from read-only plan evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrashRequest {
    pub root: PathBuf,
    pub path: PathBuf,
    pub expected_version: String,
    pub expected_parent_version: String,
    pub confirm: bool,
    pub max_bytes: u64,
}
impl TrashRequest {
    pub(super) fn plan_request(&self) -> super::super::TrashPlanRequest {
        super::super::TrashPlanRequest {
            root: self.root.clone(),
            path: self.path.clone(),
            expected_version: self.expected_version.clone(),
        }
    }
}
/// Backup construction is not removal from the selected namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrashStage {
    Preflight,
    BackingUp,
    BackedUp,
    Quarantining,
    Quarantined,
    Trashing,
    Verifying,
    Finished,
}

/// Full post-trash attributes and an explicit narrowly allowed native addition.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrashAttributes {
    pub sha256: String,
    pub bytes: usize,
    pub exact_match: bool,
    pub accepted_platform_additions: Vec<String>,
}
/// Saved evidence only; never a resume, retry, restore or permanent-delete instruction.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrashReceipt {
    pub operation: String,
    pub operation_id: String,
    pub receipt_path: String,
    pub request: TrashRequest,
    pub status: MutationStatus,
    pub stage: TrashStage,
    pub root_before: Option<FileInfo>,
    pub parent_before: Option<FileInfo>,
    pub source_before: Option<FileInfo>,
    pub trash_before: Option<FileInfo>,
    pub backup_path: Option<String>,
    pub backup_after: Option<FileInfo>,
    pub backup_verification: Option<CopyVerification>,
    pub backup_verified: bool,
    pub staging_path: Option<String>,
    pub staged_after: Option<FileInfo>,
    pub source_removed: Option<bool>,
    pub trash_attempted: bool,
    pub trash_path: Option<String>,
    pub trash_after: Option<FileInfo>,
    pub trash_attributes: Option<TrashAttributes>,
    pub root_after: Option<FileInfo>,
    pub parent_after: Option<FileInfo>,
    pub source_absent: bool,
    pub staged_source_absent: bool,
    pub mutation_attempted: bool,
    pub moved_to_trash: Option<bool>,
    pub completion_verified: bool,
    pub error: Option<FileError>,
    pub error_truncated: bool,
    pub preflight_observations_omitted: bool,
    pub provider_coordination: String,
    pub finder_put_back_supported: bool,
    pub recovery_supported: bool,
}
impl TrashReceipt {
    /// Prepare conservative one-use evidence before worker dispatch.
    pub fn new(id: String, path: String, request: TrashRequest) -> Self {
        Self {
            operation: "trash".into(),
            operation_id: id,
            receipt_path: path,
            request,
            status: MutationStatus::Uncertain,
            stage: TrashStage::Preflight,
            root_before: None,
            parent_before: None,
            source_before: None,
            trash_before: None,
            backup_path: None,
            backup_after: None,
            backup_verification: None,
            backup_verified: false,
            staging_path: None,
            staged_after: None,
            source_removed: None,
            trash_attempted: false,
            trash_path: None,
            trash_after: None,
            trash_attributes: None,
            root_after: None,
            parent_after: None,
            source_absent: false,
            staged_source_absent: false,
            mutation_attempted: false,
            moved_to_trash: None,
            completion_verified: false,
            error: None,
            error_truncated: false,
            preflight_observations_omitted: false,
            provider_coordination: "filesystem_only_provider_state_unknown".into(),
            finder_put_back_supported: false,
            recovery_supported: false,
        }
    }
    /// Reject any progressed or modified prepared record; claim separately enforces one use.
    pub fn validate_fresh(&self) -> Result<(), FileError> {
        validate_request(&self.request)?;
        let id = self.operation_id.as_bytes();
        if id.len() != 36
            || id.iter().enumerate().any(|(i, b)| {
                if [8, 13, 18, 23].contains(&i) {
                    *b != b'-'
                } else {
                    !b.is_ascii_digit() && !(b'a'..=b'f').contains(b)
                }
            })
            || !Path::new(&self.receipt_path).is_absolute()
        {
            return Err(FileError::new(
                FileErrorCode::InvalidOptions,
                "trash operation requires a canonical UUID and absolute receipt path",
            ));
        }
        let fresh = Self::new(
            self.operation_id.clone(),
            self.receipt_path.clone(),
            self.request.clone(),
        );
        if serde_json::to_value(self).map_err(json_error)?
            != serde_json::to_value(fresh).map_err(json_error)?
        {
            return Err(FileError::new(
                FileErrorCode::InvalidOptions,
                "trash receipt is not fresh; replay rejected",
            ));
        }
        Ok(())
    }
}
