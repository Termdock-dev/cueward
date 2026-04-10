use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CueSource {
    Safari,
    Notes,
    Messages,
    Ocr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentSegment {
    pub index: usize,
    pub filename: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ocr_text: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub has_ocr: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cue {
    pub source: CueSource,
    pub timestamp: DateTime<Utc>,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachment_segments: Vec<AttachmentSegment>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}
