//! Read-only selected-entry trash proposals; never remove, move or enumerate descendants.
pub mod execution;
mod model;
mod warnings;
use crate::files::scope::Scope;
use crate::files::*;
pub use model::*;
use std::path::{Component, Path};

pub const MAX_TRASH_PLAN_BYTES: usize = 64 * 1024;

/// Observe exactly one scoped entry and its parent; this plan never authorizes a write.
pub fn plan(
    platform: &impl FilePlatform,
    request: &TrashPlanRequest,
) -> Result<TrashPlan, FileError> {
    validate(request)?;
    let scope = Scope::new(platform, &request.root)?;
    let root = observe(&scope, Path::new("."), false)?;
    let resolved = scope.resolve(&request.path, false, true)?;
    let source = scope.info(&resolved)?;
    check_version(&Some(request.expected_version.clone()), &source.version)?;
    let parent = request
        .path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let source_parent = observe(&scope, parent, false)?;
    let metadata = super::metadata::inspect(&scope, &resolved, source.clone())?;
    let result = TrashPlan {
        request: request.clone(),
        target_kind: warnings::classify(&source, &metadata.resources),
        warnings: warnings::collect(&root, &source, &source_parent, &metadata.resources),
        root,
        source,
        source_parent,
        resources: metadata.resources,
        descendants_inspected: false,
        link_targets_inspected: false,
        requires_confirmation: true,
        execution_supported: false,
        recovery_supported: false,
        trash_destination: None,
        provider_coordination: "filesystem_only_provider_state_unknown".into(),
    };
    recheck(&scope, &request.path, true, &result.source)?;
    recheck(&scope, parent, false, &result.source_parent)?;
    scope.revalidate_root()?;
    bound(result)
}
fn observe(
    scope: &Scope<'_, impl FilePlatform>,
    path: &Path,
    leaf_link: bool,
) -> Result<FileInfo, FileError> {
    scope.info(&scope.resolve(path, false, leaf_link)?)
}
fn recheck(
    scope: &Scope<'_, impl FilePlatform>,
    path: &Path,
    leaf_link: bool,
    before: &FileInfo,
) -> Result<(), FileError> {
    let now = observe(scope, path, leaf_link)?;
    if now.path != before.path || now.version != before.version || now.identity != before.identity {
        return Err(FileError::new(
            FileErrorCode::Changed,
            "trash target or parent changed; observe again",
        ));
    }
    Ok(())
}
fn validate(request: &TrashPlanRequest) -> Result<(), FileError> {
    if request.expected_version.is_empty()
        || request.path.as_os_str().is_empty()
        || request
            .path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        || request
            .path
            .as_os_str()
            .to_str()
            .is_none_or(|s| s.contains('\0') || s.ends_with('/'))
    {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "trash planning requires a non-root relative entry and its observed revision",
        ));
    }
    Ok(())
}
fn bound(plan: TrashPlan) -> Result<TrashPlan, FileError> {
    if serde_json::to_vec_pretty(&plan)
        .map_err(|e| FileError::new(FileErrorCode::Internal, e.to_string()))?
        .len()
        > MAX_TRASH_PLAN_BYTES
    {
        return Err(FileError::new(
            FileErrorCode::ScanLimit,
            "trash plan exceeds 64 KiB JSON budget",
        ));
    }
    Ok(plan)
}
