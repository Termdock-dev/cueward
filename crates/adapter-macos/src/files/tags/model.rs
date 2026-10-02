use cueward_core::files::{
    FileError,
    tags::{TagEdit, TagSnapshot},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// An edit targets an explicitly observed filesystem object, not tag colors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagsRequest {
    pub root: PathBuf,
    pub path: PathBuf,
    pub expected_version: String,
    pub expected_tags_version: String,
    pub edit: TagEdit,
}
/// Parent lifetime transport names a private prepared operation only.
#[derive(Debug, Serialize, Deserialize)]
pub struct TagsWorkerRequest {
    pub operation_id: String,
}
/// A checkpoint is evidence, never an instruction to retry or undo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TagsStatus {
    Completed,
    NotStarted,
    Incomplete,
    Uncertain,
}
/// Writing is saved before submitting the single native metadata mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TagsStage {
    Preflight,
    Prepared,
    Writing,
    Verifying,
    Finished,
}
/// Retain the original bounded tag attribute for authorized manual recovery.
#[derive(Debug, Serialize, Deserialize)]
pub struct TagsReceipt {
    pub operation_id: String,
    pub receipt_path: String,
    pub request: TagsRequest,
    pub status: TagsStatus,
    pub stage: TagsStage,
    pub mutation_attempted: bool,
    pub changed_by_operation: Option<bool>,
    pub completion_verified: bool,
    pub before: Option<TagSnapshot>,
    pub after: Option<TagSnapshot>,
    pub original_attribute_base64: Option<String>,
    pub error: Option<FileError>,
    pub provider_coordination: String,
}
impl TagsReceipt {
    /// Build conservative evidence before worker dispatch.
    pub fn new(id: String, path: String, request: TagsRequest) -> Self {
        Self {
            operation_id: id,
            receipt_path: path,
            request,
            status: TagsStatus::Uncertain,
            stage: TagsStage::Preflight,
            mutation_attempted: false,
            changed_by_operation: None,
            completion_verified: false,
            before: None,
            after: None,
            original_attribute_base64: None,
            error: None,
            provider_coordination: "filesystem_only_provider_state_unknown".into(),
        }
    }
}
