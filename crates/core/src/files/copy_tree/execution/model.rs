use super::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const MAX_EXECUTION_ENTRIES: usize = 64;
pub const MAX_TREE_ATTRIBUTE_BYTES: usize = 8 * 1024 * 1024;

/// Private preparation and target publication are different stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TreeStage {
    Preflight,
    Staging,
    Copying,
    Metadata,
    Staged,
    Publishing,
    Verifying,
    Finished,
}

/// Compact node evidence; directory content hashes are intentionally absent.
#[derive(Debug, Serialize, Deserialize)]
pub struct TreeNode {
    pub relative_path: PathBuf,
    pub kind: FileKind,
    pub source_identity: String,
    pub source_version: String,
    pub source_bytes: u64,
    pub source_attributes_sha256: String,
    pub source_attribute_bytes: usize,
    pub verification: Option<CopyVerification>,
    pub destination_identity: Option<String>,
    pub destination_version: Option<String>,
}

/// Persistent conservative evidence, not a replay or cleanup instruction.
#[derive(Debug, Serialize, Deserialize)]
pub struct TreeReceipt {
    pub operation: String,
    pub operation_id: String,
    pub receipt_path: String,
    pub request: CopyTreeRequest,
    pub status: MutationStatus,
    pub stage: TreeStage,
    pub staging_path: Option<String>,
    pub staging_verified: bool,
    pub active_index: Option<usize>,
    pub nodes: Vec<TreeNode>,
    pub preflight_nodes_omitted: usize,
    pub root_before: Option<FileInfo>,
    pub parent_before: Option<FileInfo>,
    pub source_before: Option<FileInfo>,
    pub root_after: Option<FileInfo>,
    pub parent_after: Option<FileInfo>,
    pub destination_after: Option<FileInfo>,
    pub mutation_attempted: bool,
    pub destination_created: Option<bool>,
    pub completion_verified: bool,
    pub error: Option<FileError>,
    pub error_truncated: bool,
    pub provider_coordination: String,
}
impl TreeReceipt {
    /// Prepare a bounded one-use request before any worker writes.
    pub fn new(id: String, path: String, request: CopyTreeRequest) -> Self {
        Self {
            operation: "copy_tree".into(),
            operation_id: id,
            receipt_path: path,
            request,
            status: MutationStatus::Uncertain,
            stage: TreeStage::Preflight,
            staging_path: None,
            staging_verified: false,
            active_index: None,
            nodes: Vec::new(),
            preflight_nodes_omitted: 0,
            root_before: None,
            parent_before: None,
            source_before: None,
            root_after: None,
            parent_after: None,
            destination_after: None,
            mutation_attempted: false,
            destination_created: None,
            completion_verified: false,
            error: None,
            error_truncated: false,
            provider_coordination: "filesystem_only_provider_state_unknown".into(),
        }
    }
    /// Accept only a completely fresh schema; a prepared record is never resumed.
    pub fn validate_fresh(&self) -> Result<(), FileError> {
        validate_request(&self.request)?;
        if self.operation != "copy_tree"
            || self.stage != TreeStage::Preflight
            || self.status != MutationStatus::Uncertain
            || self.staging_path.is_some()
            || self.staging_verified
            || self.active_index.is_some()
            || !self.nodes.is_empty()
            || self.preflight_nodes_omitted != 0
            || self.root_before.is_some()
            || self.parent_before.is_some()
            || self.source_before.is_some()
            || self.root_after.is_some()
            || self.parent_after.is_some()
            || self.destination_after.is_some()
            || self.mutation_attempted
            || self.destination_created.is_some()
            || self.completion_verified
            || self.error.is_some()
            || self.error_truncated
        {
            return Err(FileError::new(
                FileErrorCode::InvalidOptions,
                "tree operation has progressed; replay rejected",
            ));
        }
        Ok(())
    }
}
