use crate::files::{FileError, FileInfo};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// One no-overwrite mutation within a single explicitly selected root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationRequest {
    pub root: PathBuf,
    pub destination: PathBuf,
    pub expected_parent_version: String,
    pub action: MutationAction,
}

/// A file copy requires a prior source revision; directory trees are not copied.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum MutationAction {
    Mkdir,
    Copy {
        path: PathBuf,
        expected_version: String,
        max_bytes: u64,
    },
}

/// Only completed establishes verification; incomplete can leave a partial copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationStatus {
    Completed,
    NotStarted,
    Incomplete,
    Uncertain,
}

/// Last durable checkpoint, not proof that an operation is still running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationStage {
    Preflight,
    Staging,
    Staged,
    Creating,
    Copying,
    Verifying,
    Finished,
}

/// Verified metadata preservation does not include ACLs/ownership/birth time.
#[derive(Debug, Serialize, Deserialize)]
pub struct CopyVerification {
    pub content_sha256: String,
    pub bytes: u64,
    pub permissions_equal: bool,
    pub modified_equal: bool,
    pub extended_attributes_sha256: String,
    pub extended_attributes_bytes: usize,
}

/// Durable operation evidence; non-completed results must not be replayed automatically.
#[derive(Debug, Serialize, Deserialize)]
pub struct MutationReceipt {
    pub operation_id: String,
    pub request: MutationRequest,
    pub status: MutationStatus,
    pub stage: MutationStage,
    pub mutation_attempted: bool,
    #[serde(default)]
    pub staging_path: Option<String>,
    #[serde(default)]
    pub staging_verified: bool,
    pub destination_created: Option<bool>,
    pub root_before: Option<FileInfo>,
    pub root_after: Option<FileInfo>,
    pub parent_before: Option<FileInfo>,
    pub parent_after: Option<FileInfo>,
    pub source_before: Option<FileInfo>,
    pub source_after: Option<FileInfo>,
    pub destination_before: Option<FileInfo>,
    pub destination_created_observation: Option<FileInfo>,
    pub destination_after: Option<FileInfo>,
    pub verification: Option<CopyVerification>,
    pub completion_verified: bool,
    pub error: Option<FileError>,
    pub receipt_path: String,
    pub provider_coordination: ProviderCoordination,
}

/// Descriptor filesystem operations do not establish cloud/provider coordination.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCoordination {
    FilesystemOnlyProviderStateUnknown,
}
impl MutationReceipt {
    /// Prepare conservative evidence before starting a supervised worker.
    pub fn new(operation_id: String, request: MutationRequest, receipt_path: String) -> Self {
        Self {
            operation_id,
            request,
            receipt_path,
            provider_coordination: ProviderCoordination::FilesystemOnlyProviderStateUnknown,
            status: MutationStatus::Uncertain,
            stage: MutationStage::Preflight,
            mutation_attempted: false,
            staging_path: None,
            staging_verified: false,
            destination_created: None,
            root_before: None,
            root_after: None,
            parent_before: None,
            parent_after: None,
            source_before: None,
            source_after: None,
            destination_before: None,
            destination_created_observation: None,
            destination_after: None,
            verification: None,
            completion_verified: false,
            error: None,
        }
    }
}
