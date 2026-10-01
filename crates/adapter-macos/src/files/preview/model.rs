use cueward_core::files::{FileInfo, ResourceValue};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Scoped PDF/image extraction or Quick Look thumbnail selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewRequest {
    pub root: PathBuf,
    pub path: PathBuf,
    pub follow_links: bool,
    pub expected_version: Option<String>,
    pub options: PreviewOptions,
}

/// Independent source, text, image and page bounds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewOptions {
    pub kind: PreviewKind,
    pub start_page: usize,
    pub page_count: usize,
    pub ocr: bool,
    pub render: bool,
    pub max_input_bytes: u64,
    pub max_text_bytes: usize,
    pub max_dimension: usize,
    pub max_preview_bytes: u64,
}

/// A thumbnail is a visual representation, never a document parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewKind {
    Pdf,
    Image,
    Thumbnail,
}

/// Text provenance is independent of preview-image generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextSource {
    NativePdf,
    VisionOcr,
    NotRequested,
}

/// Validated persistent PNG associated with the observed source and selected page.
#[derive(Debug, Serialize, Deserialize)]
pub struct PreviewArtifact {
    pub path: String,
    pub format: String,
    pub width: usize,
    pub height: usize,
    pub bytes: u64,
    pub sha256: String,
}

/// Selected page/frame with independent text and image availability.
#[derive(Debug, Serialize, Deserialize)]
pub struct PagePreview {
    pub page: usize,
    pub text: ResourceValue<String>,
    pub text_source: TextSource,
    pub confidence: Option<f32>,
    pub text_truncated: bool,
    pub preview: ResourceValue<PreviewArtifact>,
}

/// ImageIO metadata for the first frame; preview/OCR cover only that frame.
#[derive(Debug, Serialize, Deserialize)]
pub struct ImageInfo {
    pub content_type: String,
    pub width: usize,
    pub height: usize,
    pub frame_count: usize,
    pub orientation: Option<u32>,
}

/// Content from a bounded private snapshot, with current source checks.
#[derive(Debug, Serialize, Deserialize)]
pub struct FilePreview {
    pub file: FileInfo,
    pub source_sha256: String,
    pub kind: PreviewKind,
    pub total_pages: Option<usize>,
    pub pdf_encrypted: Option<bool>,
    pub image: Option<ImageInfo>,
    pub pages: Vec<PagePreview>,
    pub selection_truncated: bool,
    pub text_truncated: bool,
    pub cache_directory: Option<String>,
}

/// Separate native preview result in the existing external files envelope.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "operation", content = "result", rename_all = "snake_case")]
pub enum PreviewResponse {
    Preview(Box<FilePreview>),
}

/// Internal transport; parent owns the private operation directory lifetime.
#[derive(Debug, Serialize, Deserialize)]
pub struct PreviewWorkerRequest {
    pub request: PreviewRequest,
    pub directory: PathBuf,
}
