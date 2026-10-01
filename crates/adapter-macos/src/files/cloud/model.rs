use cueward_core::files::{FileError, FileInfo, MetadataError, ResourceValue};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Scoped cloud observation or explicitly requested asynchronous download.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudRequest {
    pub root: PathBuf,
    pub path: PathBuf,
    pub follow_links: bool,
    pub expected_version: Option<String>,
    pub action: CloudAction,
}

/// Downloads are opt-in and bound to a previously observed file revision.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum CloudAction {
    Status,
    Download { max_bytes: u64 },
}

/// Native iCloud resource state; an unfamiliar value is retained explicitly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum DownloadingStatus {
    NotDownloaded,
    Downloaded,
    Current,
    Other { value: String },
}

/// Per-field observations from Foundation; absent values never become false.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudResources {
    pub is_ubiquitous: ResourceValue<bool>,
    pub downloading_status: ResourceValue<DownloadingStatus>,
    pub is_downloading: ResourceValue<bool>,
    pub download_requested: ResourceValue<bool>,
    pub is_uploaded: ResourceValue<bool>,
    pub is_uploading: ResourceValue<bool>,
    pub has_unresolved_conflicts: ResourceValue<bool>,
    pub downloading_error: ResourceValue<MetadataError>,
    pub uploading_error: ResourceValue<MetadataError>,
}

/// Successful state observation is not a complete provider/readability guarantee.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudStatus {
    pub file: FileInfo,
    pub resources: CloudResources,
    pub provider_coverage: ProviderCoverage,
    pub download_requested_by_operation: bool,
}

/// Foundation ubiquitous keys do not identify all third-party providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCoverage {
    IcloudKeysOtherProvidersUnknown,
}

/// A submission receipt never establishes that current bytes were downloaded.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadDisposition {
    SentUnverified,
    AlreadyCurrent,
    AlreadyRequested,
}

/// Before/post observations distinguish a submitted request from verified data.
#[derive(Debug, Serialize, Deserialize)]
pub struct CloudDownload {
    pub operation_id: String,
    pub status: DownloadDisposition,
    pub before: CloudStatus,
    pub max_bytes: u64,
    pub download_requested_by_operation: bool,
    pub completion_verified: bool,
    pub post_check: Result<CloudStatus, FileError>,
}

/// Native cloud results use the shared external files envelope.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "operation", content = "result", rename_all = "snake_case")]
pub enum CloudResponse {
    CloudStatus(Box<CloudStatus>),
    CloudDownload(Box<CloudDownload>),
}
