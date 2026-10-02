//! Read-only batch rename proposals, never a recipe for replaying single-item writes.
mod conflicts;
mod model;
#[cfg(test)]
mod tests;
use super::{RelocationAction, RelocationPlatform, RelocationRequest};
use crate::files::scope::Scope;
use crate::files::{FileError, FileErrorCode};
pub use model::*;
use std::path::Path;

/// Conservative name comparison, without claiming the volume's exact name semantics.
pub trait BatchRenamePlatform: RelocationPlatform {
    /// Compare names without rewriting them; true can be a false positive on some volumes.
    fn names_may_collide(&self, left: &str, right: &str) -> bool;
}

/// Observe every input, retain item errors, then recheck successful proposals before returning.
pub fn plan(
    platform: &impl BatchRenamePlatform,
    request: &BatchRenameRequest,
) -> Result<BatchRenamePlan, FileError> {
    if !(1..=MAX_BATCH_RENAMES).contains(&request.entries.len()) {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "batch requires 1..64 entries",
        ));
    }
    let scope = Scope::new(platform, &request.root)?;
    let root = scope.info(&scope.resolve(Path::new("."), false, false)?)?;
    let items: Vec<_> = request
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| observe_item(platform, request, index, entry))
        .collect();
    revalidate(platform, request, &items)?;
    scope.revalidate_root()?;
    if scope
        .info(&scope.resolve(Path::new("."), false, false)?)?
        .version
        != root.version
    {
        return Err(changed());
    }
    let issues = conflicts::collect(platform, &items);
    let has_conflicts = !issues.is_empty() || items.iter().any(|item| item.error.is_some());
    Ok(BatchRenamePlan {
        request: request.clone(),
        root,
        items,
        issues,
        has_conflicts,
        execution_supported: false,
    })
}

fn observe_item(
    platform: &impl BatchRenamePlatform,
    request: &BatchRenameRequest,
    index: usize,
    entry: &BatchRenameEntry,
) -> BatchRenameItem {
    match observe(platform, request, entry) {
        Ok(proposal) => BatchRenameItem {
            index,
            proposal: Some(proposal),
            error: None,
        },
        Err(error) => BatchRenameItem {
            index,
            proposal: None,
            error: Some(error),
        },
    }
}

fn observe(
    platform: &impl BatchRenamePlatform,
    request: &BatchRenameRequest,
    entry: &BatchRenameEntry,
) -> Result<super::RelocationPlan, FileError> {
    let destination = super::rename_destination(&entry.path, Path::new(&entry.name))?;
    super::plan(
        platform,
        &RelocationRequest {
            root: request.root.clone(),
            path: entry.path.clone(),
            destination,
            expected_version: entry.expected_version.clone(),
            expected_parent_version: entry.expected_parent_version.clone(),
            action: RelocationAction::Rename,
        },
    )
}

fn revalidate(
    platform: &impl BatchRenamePlatform,
    request: &BatchRenameRequest,
    items: &[BatchRenameItem],
) -> Result<(), FileError> {
    for item in items {
        let Some(before) = &item.proposal else {
            continue;
        };
        let after = observe(platform, request, &request.entries[item.index])?;
        if before.root.version != after.root.version
            || before.source.version != after.source.version
            || before.source_parent.version != after.source_parent.version
            || before.destination_parent.version != after.destination_parent.version
            || before.destination_before.as_ref().map(|i| &i.version)
                != after.destination_before.as_ref().map(|i| &i.version)
        {
            return Err(changed());
        }
    }
    Ok(())
}

fn changed() -> FileError {
    FileError::new(
        FileErrorCode::Changed,
        "batch observations changed; reobserve before planning again",
    )
}
