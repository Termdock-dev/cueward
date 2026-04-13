use serde::Serialize;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SafariTab {
    pub window_id: i64,
    pub window_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    pub index: usize,
    pub title: String,
    pub url: String,
    pub active: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SafariEvalResult {
    pub result: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SafariReadResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    pub content: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct SafariScrollReadChunk {
    pub iteration: u64,
    pub content: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SafariScrollReadResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    pub times: u64,
    pub chunks: Vec<SafariScrollReadChunk>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SafariSourceResult {
    pub html: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SafariCloseResult {
    pub closed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SafariClickResult {
    pub clicked: bool,
    pub selector: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SafariFillResult {
    pub filled: bool,
    pub selector: String,
    pub text: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SafariWaitResult {
    pub found: bool,
    pub selector: String,
    pub timeout_seconds: u64,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SafariScrollResult {
    pub scroll_x: i64,
    pub scroll_y: i64,
}

#[derive(Debug, serde::Deserialize, PartialEq, Eq)]
pub(super) struct SafariScrollReadSnapshot {
    pub(super) item_count: usize,
    pub(super) content: String,
    #[serde(default)]
    pub(super) blocks: Vec<String>,
}
