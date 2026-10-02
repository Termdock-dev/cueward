use crate::files::{FileInfo, ResourceMetadata};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A selected entry and observed revision; neither is permission to remove it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrashPlanRequest {
    pub root: PathBuf,
    pub path: PathBuf,
    pub expected_version: String,
}

/// Resource roles are observations; unknown is never invented as false.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrashTargetKind {
    RegularFile,
    Directory,
    PackageDirectory,
    FinderAlias,
    Symlink,
    SpecialEntry,
    Unknown,
}

/// Scope/metadata warnings, not a verdict that native trashing is supported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrashWarning {
    WholeDirectoryEntry,
    SymlinkItself,
    AliasFileItself,
    AliasStateUnknown,
    PackageStateUnknown,
    SourceDataless,
    SourceAvailabilityUnknown,
    RootAvailabilityUnknown,
    ParentAvailabilityUnknown,
    SourceReadonlyMode,
    ParentReadonlyMode,
    SpecialEntry,
}

/// Bounded read-only evidence, without a trash location, receipt or execution token.
#[derive(Debug, Serialize, Deserialize)]
pub struct TrashPlan {
    pub request: TrashPlanRequest,
    pub root: FileInfo,
    pub source: FileInfo,
    pub source_parent: FileInfo,
    pub resources: ResourceMetadata,
    pub target_kind: TrashTargetKind,
    pub warnings: Vec<TrashWarning>,
    pub descendants_inspected: bool,
    pub link_targets_inspected: bool,
    pub requires_confirmation: bool,
    pub execution_supported: bool,
    pub recovery_supported: bool,
    pub trash_destination: Option<String>,
    pub provider_coordination: String,
}
