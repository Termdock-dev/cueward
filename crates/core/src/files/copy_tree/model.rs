use crate::files::{FileError, FileInfo, ResourceMetadata};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const MAX_TREE_PLAN_BYTES: usize = 1024 * 1024;
pub const MAX_TREE_ENTRIES: usize = 256;
pub const MAX_TREE_DEPTH: usize = 32;
pub const MAX_TREE_BYTES: u64 = 268435456;

/// A read-only, bounded directory copy proposal, never an execution token.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CopyTreeRequest {
    pub root: PathBuf,
    pub path: PathBuf,
    pub destination: PathBuf,
    pub expected_version: String,
    pub expected_parent_version: String,
    pub max_entries: usize,
    pub max_depth: usize,
    pub max_bytes: u64,
}

/// One observed node and exact proposed destination, in sorted depth-first order.
#[derive(Debug, Serialize, Deserialize)]
pub struct CopyTreeEntry {
    pub relative_path: PathBuf,
    pub destination: PathBuf,
    pub source: FileInfo,
    pub resources: Option<ResourceMetadata>,
    pub error: Option<FileError>,
}

/// Top-level namespace blockers; unsupported nodes retain their own error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopyTreeIssue {
    ExistingDestination,
    DestinationWithinSource,
    DifferentFilesystem,
}

/// Sequential metadata observations, without content hashes or mutation receipts.
#[derive(Debug, Serialize, Deserialize)]
pub struct CopyTreePlan {
    pub request: CopyTreeRequest,
    pub root: FileInfo,
    pub destination_parent: FileInfo,
    pub destination_before: Option<FileInfo>,
    pub entries: Vec<CopyTreeEntry>,
    pub issues: Vec<CopyTreeIssue>,
    pub observed_file_bytes: u64,
    pub enumeration_complete: bool,
    pub has_blockers: bool,
    pub execution_supported: bool,
    pub provider_coordination: String,
}
