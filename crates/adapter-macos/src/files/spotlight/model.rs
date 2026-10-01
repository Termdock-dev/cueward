use cueward_core::files::{FileError, FileInfo};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A bounded content-index query in one explicitly selected directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotlightRequest {
    pub root: PathBuf,
    pub text: String,
    pub max_depth: usize,
    pub hidden: bool,
    pub max_candidates: usize,
    pub limit: usize,
}

/// Source is always the system index, never a content scan of the filesystem.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpotlightSource {
    Spotlight,
}

/// A finished query cannot establish index coverage or freshness.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexCoverage {
    Unknown,
}

/// One in-scope index candidate, with current filesystem metadata or an error.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SpotlightEntry {
    Available {
        relative_path: String,
        file: Box<FileInfo>,
    },
    Error {
        relative_path: String,
        error: FileError,
    },
}

impl SpotlightEntry {
    /// Return the reusable root-relative path for either candidate outcome.
    pub(super) fn relative_path(&self) -> &str {
        match self {
            Self::Available { relative_path, .. } | Self::Error { relative_path, .. } => {
                relative_path
            }
        }
    }
}

/// Counts of candidates excluded without returning their paths or contents.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ExcludedCandidates {
    pub outside_scope: usize,
    pub hidden: usize,
    pub depth: usize,
    pub non_regular: usize,
    pub duplicate: usize,
}

/// Current index candidates, not proof of current content matches or absence.
#[derive(Debug, Serialize, Deserialize)]
pub struct SpotlightResult {
    pub root: FileInfo,
    pub source: SpotlightSource,
    pub query: SpotlightRequest,
    pub query_completed: bool,
    pub index_coverage: IndexCoverage,
    pub enumeration_complete: bool,
    pub content_verified: bool,
    pub candidate_count: usize,
    pub eligible_count: usize,
    pub available_count: usize,
    pub error_count: usize,
    pub excluded: ExcludedCandidates,
    pub entries: Vec<SpotlightEntry>,
    pub truncated: bool,
}

/// Tagged external result, independent of filesystem search/pagination.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "operation", content = "result", rename_all = "snake_case")]
pub enum SpotlightResponse {
    Spotlight(SpotlightResult),
}
