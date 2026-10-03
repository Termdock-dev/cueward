//! Bounded read-only tree proposals and separate verified no-overwrite execution.
mod model;
pub mod execution;
mod traversal;
use crate::files::scope::Scope;
use crate::files::*;
pub use model::*;
use std::ffi::OsString;
use std::fs::File;
use std::path::{Component, Path};

/// Guarded descriptor enumeration and platform metadata validation only.
pub trait CopyTreePlatform: FilePlatform {
    /// Open a directory without following any symlink component.
    fn open_tree_directory(&self, path: &Path) -> Result<File, FileError>;
    /// Hold a leaf link without opening its target.
    fn open_tree_link(&self, _path: &Path) -> Result<File, FileError> {
        Err(FileError::new(
            FileErrorCode::UnsupportedType,
            "link copying unavailable",
        ))
    }
    /// Enumerate the held directory, refusing more than the remaining entry budget.
    fn tree_names(&self, directory: &File, maximum: usize) -> Result<Vec<OsString>, FileError>;
    /// Check the metadata preservation subset without reading the data fork.
    fn validate_tree_metadata(&self, file: &File, info: &FileInfo) -> Result<(), FileError>;
}

/// Plan one new directory tree, retaining unsupported nodes and refusing partial scans.
pub fn plan(
    platform: &impl CopyTreePlatform,
    request: &CopyTreeRequest,
) -> Result<CopyTreePlan, FileError> {
    validate(request)?;
    let scope = Scope::new(platform, &request.root)?;
    let root = scope.info(&scope.resolve(Path::new("."), false, false)?)?;
    let source = select_source(&scope, request)?;
    let selected = select_destination(&scope, request, &source)?;
    let issues = selected.issues;
    let mut scan = traversal::Scan::new(&scope, request);
    scan.visit(&request.path, Path::new("."), 0)?;
    if scan
        .entries
        .first()
        .is_none_or(|e| e.source.version != source.version)
    {
        return Err(changed());
    }
    // Copying does not remove sources; destination-local staging supports different devices.
    scan.revalidate()?;
    recheck(&scope, &selected.path, &selected.parent, &selected.file)?;
    if destination(&scope, &request.destination)?
        .as_ref()
        .map(|i| &i.version)
        != selected.before.as_ref().map(|i| &i.version)
    {
        return Err(changed());
    }
    scope.revalidate_root()?;
    let plan = CopyTreePlan {
        request: request.clone(),
        root,
        destination_parent: selected.parent,
        destination_before: selected.before,
        has_blockers: !issues.is_empty() || scan.entries.iter().any(|e| e.error.is_some()),
        entries: scan.entries,
        issues,
        observed_file_bytes: scan.bytes,
        enumeration_complete: scan.complete,
        execution_supported: false,
        provider_coordination: "filesystem_only_provider_state_unknown".into(),
    };
    bound_plan(plan)
}
fn bound_plan(plan: CopyTreePlan) -> Result<CopyTreePlan, FileError> {
    if serde_json::to_vec_pretty(&plan)
        .map_err(|e| FileError::new(FileErrorCode::Internal, e.to_string()))?
        .len()
        > MAX_TREE_PLAN_BYTES
    {
        return Err(FileError::new(
            FileErrorCode::ScanLimit,
            "copy-tree plan exceeds 1 MiB JSON budget",
        ));
    }
    Ok(plan)
}

fn select_source(
    scope: &Scope<'_, impl FilePlatform>,
    request: &CopyTreeRequest,
) -> Result<FileInfo, FileError> {
    let source = scope.info(&scope.resolve(&request.path, false, true)?)?;
    check_version(&Some(request.expected_version.clone()), &source.version)?;
    if source.kind != FileKind::Directory {
        return Err(FileError::new(
            FileErrorCode::UnsupportedType,
            "copy-tree requires a directory",
        ));
    }
    Ok(source)
}
struct Destination {
    path: std::path::PathBuf,
    parent: FileInfo,
    file: File,
    before: Option<FileInfo>,
    issues: Vec<CopyTreeIssue>,
}
fn select_destination(
    scope: &Scope<'_, impl CopyTreePlatform>,
    request: &CopyTreeRequest,
    source: &FileInfo,
) -> Result<Destination, FileError> {
    let parent_path = request
        .destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = scope.info(&scope.resolve(parent_path, false, false)?)?;
    let parent_file = traversal::open(scope.platform, &parent)?;
    check_version(
        &Some(request.expected_parent_version.clone()),
        &parent.version,
    )?;
    let destination_before = destination(scope, &request.destination)?;
    let mut issues = Vec::new();
    if destination_before.is_some() {
        issues.push(CopyTreeIssue::ExistingDestination);
    }
    if parent_within_source(scope, parent_path, &source.identity)? {
        issues.push(CopyTreeIssue::DestinationWithinSource);
    }
    Ok(Destination {
        path: parent_path.into(),
        parent,
        file: parent_file,
        before: destination_before,
        issues,
    })
}

pub(super) fn parent_within_source(
    scope: &Scope<'_, impl FilePlatform>,
    parent: &Path,
    source_id: &str,
) -> Result<bool, FileError> {
    let mut path = parent;
    loop {
        if scope.info(&scope.resolve(path, false, false)?)?.identity == source_id {
            return Ok(true);
        }
        match path.parent() {
            Some(next) if !next.as_os_str().is_empty() => path = next,
            _ if path != Path::new(".") => path = Path::new("."),
            _ => return Ok(false),
        }
    }
}

fn destination(
    scope: &Scope<'_, impl FilePlatform>,
    path: &Path,
) -> Result<Option<FileInfo>, FileError> {
    match scope.resolve(path, false, true) {
        Ok(resolved) => Ok(Some(scope.info(&resolved)?)),
        Err(error) if error.code == FileErrorCode::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn validate(request: &CopyTreeRequest) -> Result<(), FileError> {
    if !(1..=MAX_TREE_ENTRIES).contains(&request.max_entries)
        || !(1..=MAX_TREE_DEPTH).contains(&request.max_depth)
        || !(1..=MAX_TREE_BYTES).contains(&request.max_bytes)
        || request.expected_version.is_empty()
        || request.expected_parent_version.is_empty()
        || request.destination.as_os_str().is_empty()
        || request
            .destination
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        || request
            .destination
            .as_os_str()
            .to_str()
            .is_none_or(|s| s.contains('\0') || s.ends_with('/'))
    {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "copy-tree requires source/parent revisions, a new relative destination and bounded entries/depth/bytes",
        ));
    }
    Ok(())
}

fn recheck(
    scope: &Scope<'_, impl FilePlatform>,
    path: &Path,
    before: &FileInfo,
    file: &File,
) -> Result<(), FileError> {
    let now = scope.info(&scope.resolve(path, false, true)?)?;
    if now.path != before.path
        || now.version != before.version
        || scope.platform.stamp(&file.metadata()?).version != before.version
    {
        return Err(changed());
    }
    Ok(())
}
fn changed() -> FileError {
    FileError::new(
        FileErrorCode::Changed,
        "copy-tree observations changed; reobserve before planning again",
    )
}
