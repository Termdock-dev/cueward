use super::super::{BatchRenameIssue, BatchRenameRequest};
use crate::files::relocation::{RelocationReceipt, RelocationStatus};
use crate::files::{FileError, FileErrorCode, FileInfo};
use serde::{Deserialize, Serialize};

/// Keep quadratic conflict diagnostics from exhausting the bounded aggregate store.
pub const MAX_STORED_BATCH_ISSUES: usize = 128;

/// Aggregate progress; individual native evidence stays in relocation receipts.
#[derive(Debug, Serialize, Deserialize)]
pub struct BatchExecutionReceipt {
    pub operation_id: String,
    pub receipt_path: String,
    pub request: BatchRenameRequest,
    pub status: RelocationStatus,
    pub completion_verified: bool,
    pub started: bool,
    pub root: Option<FileInfo>,
    pub active_index: Option<usize>,
    pub items: Vec<BatchExecutionItem>,
    pub issues: Vec<BatchRenameIssue>,
    /// Omitted diagnostics, not omitted inputs; older receipts kept their full issue list.
    #[serde(default)]
    pub omitted_issue_count: usize,
    pub error: Option<FileError>,
}
/// A pending item has no child ID; an allocated child is never replayed.
#[derive(Debug, Serialize, Deserialize)]
pub struct BatchExecutionItem {
    pub index: usize,
    pub operation_id: Option<String>,
    pub receipt_path: Option<String>,
    pub status: Option<RelocationStatus>,
    pub mutation_attempted: bool,
    pub renamed: Option<bool>,
    pub completion_verified: bool,
    pub error: Option<FileError>,
}
impl BatchExecutionReceipt {
    /// Persist conservative aggregate evidence before dispatching the one-use worker.
    pub fn new(id: String, path: String, request: BatchRenameRequest) -> Self {
        let items = (0..request.entries.len())
            .map(|index| BatchExecutionItem {
                index,
                operation_id: None,
                receipt_path: None,
                status: None,
                mutation_attempted: false,
                renamed: None,
                completion_verified: false,
                error: None,
            })
            .collect();
        Self {
            operation_id: id,
            receipt_path: path,
            request,
            status: RelocationStatus::Uncertain,
            completion_verified: false,
            started: false,
            root: None,
            active_index: None,
            items,
            issues: Vec::new(),
            omitted_issue_count: 0,
            error: None,
        }
    }
    /// Reject malformed or progressed private records before indexing or invoking a child.
    pub fn validate_initial(&self) -> Result<(), FileError> {
        if self.status != RelocationStatus::Uncertain
            || self.started
            || self.completion_verified
            || self.root.is_some()
            || self.active_index.is_some()
            || self.error.is_some()
            || !self.issues.is_empty()
            || self.omitted_issue_count != 0
            || self.items.len() != self.request.entries.len()
            || self.items.iter().enumerate().any(|(index, item)| {
                item.index != index
                    || item.operation_id.is_some()
                    || item.receipt_path.is_some()
                    || item.status.is_some()
                    || item.error.is_some()
                    || item.mutation_attempted
                    || item.renamed.is_some()
                    || item.completion_verified
            })
        {
            return Err(FileError::new(
                FileErrorCode::InvalidOptions,
                "batch record is malformed or has progressed; replay rejected",
            ));
        }
        Ok(())
    }
    /// Mirror saved child evidence without treating an allocation as a submitted rename.
    pub fn observe_child(&mut self, index: usize, child: &RelocationReceipt) {
        let item = &mut self.items[index];
        item.operation_id = Some(child.operation_id.clone());
        item.receipt_path = Some(child.receipt_path.clone());
        item.status = Some(child.status);
        item.mutation_attempted = child.mutation_attempted;
        item.renamed = child.renamed;
        item.completion_verified = child.completion_verified;
        item.error = child
            .error
            .as_ref()
            .map(|e| FileError::new(e.code, &e.message));
    }
    /// Stop on the first failure; never turn partial or unknown progress into completion.
    pub fn record_error(&mut self, error: FileError) {
        self.status = if self
            .items
            .iter()
            .any(|i| i.mutation_attempted && i.renamed.is_none())
        {
            RelocationStatus::Uncertain
        } else if self
            .items
            .iter()
            .any(|i| i.renamed == Some(true) || i.completion_verified)
        {
            RelocationStatus::Incomplete
        } else {
            RelocationStatus::NotStarted
        };
        self.completion_verified = false;
        self.error = Some(error);
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
