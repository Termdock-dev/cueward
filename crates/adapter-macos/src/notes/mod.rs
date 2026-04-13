use std::path::PathBuf;

use crate::MacosError;

pub mod crud;
pub mod notify;
mod attachments;
mod capture;
mod db;

pub use capture::capture;

const ATTACHMENT_PLACEHOLDER: char = '\u{fffc}';
const ATTACHMENT_LABEL: &str = "[Attachment]";
const MEDIA_MATCH_WINDOW_SECS: i64 = 5;
const APPLE_EPOCH_OFFSET: f64 = 978_307_200.0;
const MAX_MEDIA_SEARCH_DEPTH: usize = 10;
const OCR_EMPTY_SENTINEL: &str = "__CUEWARD_OCR_EMPTY__";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MediaAttachment {
    pub(crate) filename: String,
    pub(crate) path: PathBuf,
    pub(crate) sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MediaNote {
    pub(crate) timestamp: i64,
    pub(crate) title: Option<String>,
    pub(crate) attachments: Vec<MediaAttachment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WebPreviewAttachment {
    pub(crate) title: Option<String>,
    pub(crate) url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WebPreviewNote {
    pub(crate) timestamp: i64,
    pub(crate) title: Option<String>,
    pub(crate) attachments: Vec<WebPreviewAttachment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AttachmentOcrBlock {
    pub(crate) index: usize,
    pub(crate) filename: String,
    pub(crate) sha256: Option<String>,
    pub(crate) text: String,
}

fn home_dir() -> Result<PathBuf, MacosError> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| MacosError::Other("HOME environment variable must be set".into()))
}
