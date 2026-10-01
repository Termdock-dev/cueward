use cueward_core::files::{FileError, FileInfo};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct FinderRequest {
    pub root: PathBuf,
    pub action: FinderAction,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum FinderAction {
    Context {
        max_items: usize,
    },
    Reveal {
        path: PathBuf,
        follow_links: bool,
        expected_version: Option<String>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "operation", content = "result", rename_all = "snake_case")]
pub enum FinderResponse {
    FinderContext(FinderContext),
    FinderReveal(FinderReveal),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum FinderItem {
    Available {
        relative_path: String,
        file: Box<FileInfo>,
    },
    OutsideScope,
    Error {
        error: FileError,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FinderWindow {
    pub id: i64,
    pub location: FinderItem,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ForegroundObservation {
    pub frontmost_pid_before: Option<i32>,
    pub frontmost_pid_after: Option<i32>,
    pub foreground_changed: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FinderContext {
    pub root: FileInfo,
    pub finder_pid: Option<i32>,
    pub window_count: Option<usize>,
    pub front_window: Option<FinderWindow>,
    pub selection: Option<Vec<FinderItem>>,
    pub selection_complete: bool,
    pub activation_requested: bool,
    pub foreground: ForegroundObservation,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevealStatus {
    SentUnverified,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FinderReveal {
    pub status: RevealStatus,
    pub file: FileInfo,
    pub activation_requested: bool,
    pub selection_change_requested: bool,
    pub foreground: ForegroundObservation,
    pub post_check: Result<FileInfo, FileError>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RawContext {
    pub window_count: usize,
    pub front_window: Option<RawWindow>,
    pub selection: Vec<RawItem>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RawWindow {
    pub id: i64,
    pub location: RawItem,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(super) enum RawItem {
    Available { url: String },
    Error { error: FileError },
}
