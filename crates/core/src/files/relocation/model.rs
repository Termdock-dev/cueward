use crate::files::{FileInfo, ResourceMetadata};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Observe a selected source and a proposed destination; this request does not authorize replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelocationRequest {
    pub root: PathBuf,
    pub path: PathBuf,
    pub destination: PathBuf,
    pub expected_version: String,
    pub expected_parent_version: String,
    pub action: RelocationAction,
}

/// Rename keeps the parent; move selects an explicit destination under the same root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelocationAction {
    Rename,
    Move,
}

/// Read-only observations, not a completed mutation or a reusable execution token.
#[derive(Debug, Serialize, Deserialize)]
pub struct RelocationPlan {
    pub request: RelocationRequest,
    pub root: FileInfo,
    pub source: FileInfo,
    pub source_parent: FileInfo,
    pub source_resources: ResourceMetadata,
    pub destination_parent: FileInfo,
    pub destination_path: String,
    pub destination_before: Option<FileInfo>,
    pub no_op: bool,
    pub same_filesystem: bool,
}
