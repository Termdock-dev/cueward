use std::path::PathBuf;

use cueward_core::AttachmentKind;

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

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MapAttachment {
    pub(crate) title: Option<String>,
    pub(crate) url: Option<String>,
    pub(crate) latitude: f64,
    pub(crate) longitude: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MapNote {
    pub(crate) timestamp: i64,
    pub(crate) title: Option<String>,
    pub(crate) attachments: Vec<MapAttachment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileBackedAttachment {
    pub(crate) kind: AttachmentKind,
    pub(crate) title: Option<String>,
    pub(crate) filename: String,
    pub(crate) path: PathBuf,
    pub(crate) sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileBackedNote {
    pub(crate) timestamp: i64,
    pub(crate) title: Option<String>,
    pub(crate) attachments: Vec<FileBackedAttachment>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AudioAttachment {
    pub(crate) title: Option<String>,
    pub(crate) filename: String,
    pub(crate) path: PathBuf,
    pub(crate) sha256: Option<String>,
    pub(crate) duration_seconds: Option<f64>,
    pub(crate) transcript_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AudioNote {
    pub(crate) timestamp: i64,
    pub(crate) title: Option<String>,
    pub(crate) attachments: Vec<AudioAttachment>,
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
