use crate::files::relocation::RelocationPlan;
use crate::files::{FileError, FileInfo};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const MAX_BATCH_RENAMES: usize = 64;

/// One explicitly selected sibling rename; no templates or automatic suffixes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchRenameEntry {
    pub path: PathBuf,
    pub name: String,
    pub expected_version: String,
    pub expected_parent_version: String,
    /// Rename only this leaf symlink object; missing/false keeps legacy link rejection.
    #[serde(default, skip_serializing_if = "is_false")]
    pub link_itself: bool,
}

/// Bounded read-only proposals under one selected root, in input order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchRenameRequest {
    pub root: PathBuf,
    pub entries: Vec<BatchRenameEntry>,
}

/// An item failure is not an empty source or an omitted batch entry.
#[derive(Debug, Serialize, Deserialize)]
pub struct BatchRenameItem {
    pub index: usize,
    pub proposal: Option<RelocationPlan>,
    pub error: Option<FileError>,
}

/// Observed conflicts and conservative cross-entry dependencies; no execution order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchRenameIssueCode {
    ExistingDestination,
    DifferentFilesystem,
    DuplicateSource,
    SharedSourceIdentity,
    DuplicateDestination,
    PotentialDestinationCollision,
    SourceDestinationDependency,
    NestedSource,
}

/// Zero-based item indices involved in an issue; a single-item issue has one index.
#[derive(Debug, Serialize, Deserialize)]
pub struct BatchRenameIssue {
    pub code: BatchRenameIssueCode,
    pub entries: Vec<usize>,
}

/// Observations only: no receipts, execution tokens, namespace lock or mutation success.
#[derive(Debug, Serialize, Deserialize)]
pub struct BatchRenamePlan {
    pub request: BatchRenameRequest,
    pub root: FileInfo,
    pub items: Vec<BatchRenameItem>,
    pub issues: Vec<BatchRenameIssue>,
    pub has_conflicts: bool,
    pub execution_supported: bool,
}

fn is_false(value: &bool) -> bool {
    !value
}
