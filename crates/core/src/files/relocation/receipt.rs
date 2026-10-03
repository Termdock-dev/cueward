use super::*;
use serde::{Deserialize, Serialize};

/// Native outcome distinguishes a rejected operation from an uncertain submission.
pub struct RenameOutcome {
    pub result: Result<(), FileError>,
    pub renamed: Option<bool>,
}
/// Evidence is never a replay or rollback instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelocationStatus {
    Completed,
    NotStarted,
    Incomplete,
    Uncertain,
}
/// Renaming is saved before the native call, verifying after its return.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelocationStage {
    Preflight,
    Prepared,
    Renaming,
    Verifying,
    Finished,
}
/// Observed paths and identities; completion means checked observations, not isolation.
#[derive(Debug, Serialize, Deserialize)]
pub struct RelocationReceipt {
    pub operation_id: String,
    pub receipt_path: String,
    pub request: RelocationRequest,
    pub status: RelocationStatus,
    pub stage: RelocationStage,
    pub mutation_attempted: bool,
    pub renamed: Option<bool>,
    pub completion_verified: bool,
    pub before: Option<RelocationPlan>,
    pub root_after: Option<FileInfo>,
    pub source_parent_after: Option<FileInfo>,
    pub destination_parent_after: Option<FileInfo>,
    pub source_after: Option<FileInfo>,
    pub source_path_absent: Option<bool>,
    pub destination_after: Option<FileInfo>,
    pub error: Option<FileError>,
    pub provider_coordination: String,
    /// Different-device moves publish an independent verified copy and retain the original.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy_operation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copied: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retained_source_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_quarantined: Option<bool>,
}
impl RelocationReceipt {
    /// Allocate conservative initial evidence before starting a one-use worker.
    pub fn new(id: String, path: String, request: RelocationRequest) -> Self {
        Self {
            operation_id: id,
            receipt_path: path,
            request,
            status: RelocationStatus::Uncertain,
            stage: RelocationStage::Preflight,
            mutation_attempted: false,
            renamed: None,
            completion_verified: false,
            before: None,
            root_after: None,
            source_parent_after: None,
            destination_parent_after: None,
            source_after: None,
            source_path_absent: None,
            destination_after: None,
            error: None,
            copy_operation_id: None,
            copied: None,
            retained_source_path: None,
            source_quarantined: None,
            provider_coordination: "filesystem_only_provider_state_unknown".into(),
        }
    }
    /// Preserve actual evidence and classify failure without retrying or undoing.
    pub fn record_error(&mut self, error: FileError) {
        self.status = if self.copy_operation_id.is_some()
            && self.copied != Some(false)
            && self.mutation_attempted
        {
            if self.copied == Some(true) {
                RelocationStatus::Incomplete
            } else {
                RelocationStatus::Uncertain
            }
        } else {
            match self.renamed {
                Some(true) => RelocationStatus::Incomplete,
                Some(false) => RelocationStatus::NotStarted,
                None if !self.mutation_attempted => RelocationStatus::NotStarted,
                None => RelocationStatus::Uncertain,
            }
        };
        self.completion_verified = false;
        self.error = Some(error);
    }
}
