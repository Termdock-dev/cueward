use serde::{Deserialize, Serialize};
use std::fmt;
use std::io;
use std::path::PathBuf;

pub const MAX_READ_BYTES: usize = 1024 * 1024;
pub const MAX_SCAN_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_DIRECTORY_ENTRIES: usize = 10_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRequest {
    pub root: PathBuf,
    pub path: PathBuf,
    pub follow_links: bool,
    pub expected_version: Option<String>,
    pub action: FileAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", content = "options", rename_all = "snake_case")]
pub enum FileAction {
    Info,
    Metadata,
    List(ListOptions),
    Read(ReadOptions),
    Search(super::SearchOptions),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListOptions {
    pub limit: usize,
    pub offset: usize,
    pub hidden: bool,
    pub sort: FileSort,
    pub descending: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileSort {
    Name,
    Size,
    Modified,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadOptions {
    pub offset: u64,
    pub max_bytes: usize,
    pub encoding: FileEncoding,
    pub start_line: Option<u64>,
    pub line_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileEncoding {
    Utf8,
    Utf16Le,
    Utf16Be,
    Hex,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    File,
    Directory,
    Symlink,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataState {
    NotDataless,
    Dataless,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStamp {
    pub identity: String,
    pub version: String,
    pub data_state: DataState,
    pub mode: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub requested_path: String,
    pub path: String,
    pub resolved_path: Option<String>,
    pub name: String,
    pub kind: FileKind,
    pub identity: String,
    pub version: String,
    pub size: u64,
    pub modified: Option<String>,
    pub created: Option<String>,
    pub readonly: bool,
    pub mode: Option<u32>,
    pub data_state: DataState,
    pub link_target: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileListing {
    pub directory: FileInfo,
    pub version: String,
    pub entries: Vec<FileInfo>,
    pub total: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
    pub enumeration_complete: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileRead {
    pub file: FileInfo,
    pub encoding: FileEncoding,
    pub content: String,
    pub offset: u64,
    pub bytes_read: usize,
    pub next_offset: Option<u64>,
    pub eof: bool,
    pub truncated: bool,
    pub start_line: Option<u64>,
    pub lines_complete: Option<usize>,
    pub next_line: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "operation", content = "result", rename_all = "snake_case")]
pub enum FileResponse {
    Info(FileInfo),
    Metadata(super::FileMetadata),
    List(FileListing),
    Read(FileRead),
    Search(super::FileSearch),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileErrorCode {
    InvalidOptions,
    NotFound,
    PermissionDenied,
    OutsideRoot,
    SymlinkDisallowed,
    UnsupportedType,
    UnsupportedPathEncoding,
    DecodeError,
    Encrypted,
    CorruptData,
    BinaryData,
    ScanLimit,
    Unavailable,
    Changed,
    Conflict,
    VerificationFailed,
    Timeout,
    Io,
    Internal,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileError {
    pub code: FileErrorCode,
    pub message: String,
}

impl FileError {
    /// Construct a machine-readable file operation failure.
    pub fn new(code: FileErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl From<io::Error> for FileError {
    fn from(error: io::Error) -> Self {
        let code = match error.kind() {
            io::ErrorKind::NotFound => FileErrorCode::NotFound,
            io::ErrorKind::PermissionDenied => FileErrorCode::PermissionDenied,
            io::ErrorKind::TimedOut => FileErrorCode::Timeout,
            io::ErrorKind::AlreadyExists => FileErrorCode::Conflict,
            _ => FileErrorCode::Io,
        };
        Self::new(code, error.to_string())
    }
}

impl fmt::Display for FileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}
impl std::error::Error for FileError {}
