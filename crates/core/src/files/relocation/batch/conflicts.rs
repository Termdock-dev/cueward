use super::*;
use crate::files::FileKind;
use crate::files::relocation::RelocationPlan;

/// Report only observed single-item conflicts and conservative successful-item interactions.
pub(super) fn collect(
    platform: &impl BatchRenamePlatform,
    items: &[BatchRenameItem],
) -> Vec<BatchRenameIssue> {
    let mut issues = Vec::new();
    for (position, item) in items.iter().enumerate() {
        let Some(plan) = &item.proposal else { continue };
        if plan.destination_before.is_some() && !plan.no_op {
            add(
                &mut issues,
                BatchRenameIssueCode::ExistingDestination,
                &[item.index],
            );
        }
        if !plan.same_filesystem {
            add(
                &mut issues,
                BatchRenameIssueCode::DifferentFilesystem,
                &[item.index],
            );
        }
        for other in &items[position + 1..] {
            if let Some(right) = &other.proposal {
                pair(
                    platform,
                    plan,
                    right,
                    &[item.index, other.index],
                    &mut issues,
                );
            }
        }
    }
    issues
}

fn pair(
    platform: &impl BatchRenamePlatform,
    left: &RelocationPlan,
    right: &RelocationPlan,
    indices: &[usize],
    issues: &mut Vec<BatchRenameIssue>,
) {
    if left.source.path == right.source.path {
        add(issues, BatchRenameIssueCode::DuplicateSource, indices);
    } else if left.source.identity == right.source.identity {
        add(issues, BatchRenameIssueCode::SharedSourceIdentity, indices);
    }
    if left.destination_parent.identity == right.destination_parent.identity {
        let a = leaf(&left.destination_path);
        let b = leaf(&right.destination_path);
        if a == b {
            add(issues, BatchRenameIssueCode::DuplicateDestination, indices);
        } else if platform.names_may_collide(a, b) {
            add(
                issues,
                BatchRenameIssueCode::PotentialDestinationCollision,
                indices,
            );
        }
    }
    if depends_on(platform, left, right) || depends_on(platform, right, left) {
        add(
            issues,
            BatchRenameIssueCode::SourceDestinationDependency,
            indices,
        );
    }
    if contains(platform, left, right) || contains(platform, right, left) {
        add(issues, BatchRenameIssueCode::NestedSource, indices);
    }
}

fn depends_on(platform: &impl BatchRenamePlatform, a: &RelocationPlan, b: &RelocationPlan) -> bool {
    a.destination_parent.identity == b.source_parent.identity
        && platform.names_may_collide(leaf(&a.destination_path), leaf(&b.source.path))
}

fn contains(
    platform: &impl BatchRenamePlatform,
    directory: &RelocationPlan,
    other: &RelocationPlan,
) -> bool {
    if directory.source.kind != FileKind::Directory {
        return false;
    }
    let source: Vec<_> = Path::new(&directory.source.path).components().collect();
    let parent: Vec<_> = Path::new(&other.source_parent.path).components().collect();
    source.len() <= parent.len()
        && source.iter().zip(&parent).all(|(a, b)| {
            match (a.as_os_str().to_str(), b.as_os_str().to_str()) {
                (Some(a), Some(b)) => platform.names_may_collide(a, b),
                _ => false, // Successful plans have UTF-8 paths; never compare lossy names.
            }
        })
}

fn leaf(path: &str) -> &str {
    // Successful relocation plans have one validated normal leaf.
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
}

fn add(issues: &mut Vec<BatchRenameIssue>, code: BatchRenameIssueCode, indices: &[usize]) {
    issues.push(BatchRenameIssue {
        code,
        entries: indices.to_vec(),
    });
}
