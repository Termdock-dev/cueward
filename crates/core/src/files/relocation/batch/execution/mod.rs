//! Fail-stop execution of independent proposals, with revision updates from verified writes only.
mod model;
mod verification;
use super::{BatchRenamePlan, BatchRenamePlatform, BatchRenameRequest};
use crate::files::relocation::{RelocationReceipt, RelocationRequest, RelocationStatus};
use crate::files::{FileError, FileErrorCode, FileInfo};
pub use model::*;

/// Replan pending sources, checkpoint each child, and stop without retry or rollback.
pub fn execute<P: BatchRenamePlatform>(
    platform: &P,
    receipt: &mut BatchExecutionReceipt,
    checkpoint: &mut impl FnMut(&BatchExecutionReceipt) -> Result<(), FileError>,
    invoke: &mut impl FnMut(
        usize,
        &RelocationRequest,
        &FileInfo,
        &mut BatchExecutionReceipt,
    ) -> Result<RelocationReceipt, FileError>,
) -> Result<(), FileError> {
    receipt.validate_initial()?;
    receipt.started = true;
    checkpoint(receipt)?;
    let mut plan = super::plan(platform, &receipt.request)?;
    receipt.root = Some(plan.root.clone());
    reject_conflicts(receipt, &mut plan, 0)?;
    let held_root = platform.open_directory(std::path::Path::new(&plan.root.path))?;
    let mut root = plan.root.clone();
    let mut completed = Vec::new();
    checkpoint(receipt)?;
    for index in 0..receipt.items.len() {
        verification::root(platform, &held_root, &receipt.request.root, &root)?;
        let request = next_request(platform, &plan, &root, receipt, index)?;
        receipt.active_index = Some(index);
        checkpoint(receipt)?;
        let child = invoke(index, &request, &root, receipt)?;
        receipt.observe_child(index, &child);
        checkpoint(receipt)?;
        require_completion(&child)?;
        update_revisions(&mut plan, &mut root, &child)?;
        completed.push(child);
    }
    verification::finish(
        platform,
        &held_root,
        &receipt.request.root,
        &root,
        &plan,
        &completed,
    )?;
    receipt.active_index = None;
    receipt.status = RelocationStatus::Completed;
    receipt.completion_verified = true;
    checkpoint(receipt)
}
fn next_request(
    platform: &impl BatchRenamePlatform,
    plan: &BatchRenamePlan,
    root: &FileInfo,
    receipt: &mut BatchExecutionReceipt,
    index: usize,
) -> Result<RelocationRequest, FileError> {
    let mut fresh = super::plan(platform, &pending_request(plan, index)?)?;
    verification::same(&fresh.root, root)?;
    reject_conflicts(receipt, &mut fresh, index)?;
    Ok(fresh.items[0]
        .proposal
        .as_ref()
        .ok_or_else(|| changed("missing proposal"))?
        .request
        .clone())
}
fn require_completion(child: &RelocationReceipt) -> Result<(), FileError> {
    if child.status != RelocationStatus::Completed || !child.completion_verified {
        return Err(child
            .error
            .as_ref()
            .map(|e| FileError::new(e.code, &e.message))
            .unwrap_or_else(|| changed("child completion was not verified")));
    }
    Ok(())
}

fn pending_request(plan: &BatchRenamePlan, start: usize) -> Result<BatchRenameRequest, FileError> {
    let mut request = plan.request.clone();
    request.root = plan.root.path.clone().into();
    request.entries = plan.items[start..]
        .iter()
        .map(|item| {
            let mut entry = plan.request.entries[item.index].clone();
            entry.expected_parent_version = item
                .proposal
                .as_ref()
                .ok_or_else(|| changed("missing parent revision"))?
                .source_parent
                .version
                .clone();
            Ok(entry)
        })
        .collect::<Result<_, FileError>>()?;
    Ok(request)
}
fn reject_conflicts(
    receipt: &mut BatchExecutionReceipt,
    plan: &mut BatchRenamePlan,
    start: usize,
) -> Result<(), FileError> {
    if !plan.has_conflicts {
        return Ok(());
    }
    for item in &mut plan.items {
        receipt.items[start + item.index].error = item.error.take();
    }
    receipt.issues = std::mem::take(&mut plan.issues);
    for issue in &mut receipt.issues {
        for index in &mut issue.entries {
            *index += start;
        }
    }
    Err(FileError::new(
        FileErrorCode::Conflict,
        "batch has item errors or conflicts; no further rename submitted",
    ))
}
fn update_revisions(
    plan: &mut BatchRenamePlan,
    root: &mut FileInfo,
    child: &RelocationReceipt,
) -> Result<(), FileError> {
    let after = child
        .root_after
        .as_ref()
        .ok_or_else(|| changed("missing root after rename"))?;
    if after.identity != root.identity || after.path != root.path {
        return Err(changed("root identity changed"));
    }
    *root = after.clone();
    let parent = child
        .source_parent_after
        .as_ref()
        .ok_or_else(|| changed("missing parent after rename"))?;
    for item in &mut plan.items {
        let proposal = item
            .proposal
            .as_mut()
            .ok_or_else(|| changed("missing initial proposal"))?;
        if proposal.source_parent.identity == parent.identity {
            if proposal.source_parent.path != parent.path {
                return Err(changed("parent path changed"));
            }
            proposal.source_parent = parent.clone();
        }
    }
    Ok(())
}
fn changed(message: &str) -> FileError {
    FileError::new(FileErrorCode::Changed, message)
}
