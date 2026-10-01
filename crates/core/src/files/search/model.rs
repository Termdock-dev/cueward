use super::super::{FileInfo, FileKind, MAX_DIRECTORY_ENTRIES};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const MAX_SEARCH_DEPTH: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchOptions {
    pub name: Option<String>,
    pub kind: Option<FileKind>,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub modified_after: Option<DateTime<Utc>>,
    pub modified_before: Option<DateTime<Utc>>,
    pub hidden: bool,
    pub max_depth: usize,
    pub max_entries: usize,
    pub limit: usize,
    pub offset: usize,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            name: None,
            kind: None,
            min_size: None,
            max_size: None,
            modified_after: None,
            modified_before: None,
            hidden: false,
            max_depth: 1,
            max_entries: MAX_DIRECTORY_ENTRIES,
            limit: 100,
            offset: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchEntry {
    /// Path relative to the canonical root, suitable for a later files info/read.
    pub relative_path: String,
    pub file: FileInfo,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchSource {
    Filesystem,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileSearch {
    pub directory: FileInfo,
    pub source: SearchSource,
    pub query: SearchOptions,
    pub version: String,
    pub entries: Vec<SearchEntry>,
    pub total: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
    /// Complete only within query.max_depth and its hidden/symlink policy.
    pub enumeration_complete: bool,
    pub entries_observed: usize,
    pub directories_scanned: usize,
    /// Visible directories whose children are outside the selected depth.
    pub depth_boundary_directories: usize,
}
