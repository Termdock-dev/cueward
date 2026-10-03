use super::*;
use crate::files::relocation::batch::{BatchRenameEntry, BatchRenameIssueCode};
fn fresh() -> BatchExecutionReceipt {
    BatchExecutionReceipt::new(
        "id".into(),
        "receipt".into(),
        BatchRenameRequest {
            root: "/owned".into(),
            entries: vec![BatchRenameEntry {
                path: "a".into(),
                name: "new".into(),
                expected_version: "source".into(),
                expected_parent_version: "parent".into(),
                link_itself: false,
            }],
        },
    )
}
#[test]
fn batch_execution_old_receipt_keeps_complete_issues_and_defaults_to_zero_omissions() {
    let mut receipt = fresh();
    receipt.issues.push(BatchRenameIssue {
        code: BatchRenameIssueCode::ExistingDestination,
        entries: vec![0],
    });
    receipt.status = RelocationStatus::NotStarted;
    let mut old = serde_json::to_value(receipt).unwrap();
    old.as_object_mut().unwrap().remove("omitted_issue_count");
    let loaded: BatchExecutionReceipt = serde_json::from_value(old).unwrap();
    assert_eq!(loaded.omitted_issue_count, 0);
    assert_eq!(loaded.issues.len(), 1);
    assert_eq!(
        loaded.issues[0].code,
        BatchRenameIssueCode::ExistingDestination
    );
    assert_eq!(loaded.issues[0].entries, vec![0]);
    assert_eq!(loaded.status, RelocationStatus::NotStarted);
}
#[test]
fn batch_execution_omission_marker_is_not_a_fresh_replayable_request() {
    let mut receipt = fresh();
    assert!(receipt.validate_initial().is_ok());
    receipt.omitted_issue_count = 1;
    assert_eq!(
        receipt.validate_initial().unwrap_err().code,
        FileErrorCode::InvalidOptions
    );
}
