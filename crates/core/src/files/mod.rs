//! Scoped filesystem reads and no-overwrite mutations with platform identity/open hooks.
mod listing;
mod metadata;
mod model;
pub mod mutation;
pub mod relocation;
pub mod tags;
mod reading;
mod scope;
mod search;

pub use metadata::{FileMetadata, MetadataError, ResourceMetadata, ResourceValue};
pub use model::*;
pub use search::{FileSearch, MAX_SEARCH_DEPTH, SearchEntry, SearchOptions, SearchSource};

use std::fs::{File, Metadata};
use std::io;
use std::path::Path;

/// Platform identity and guarded-open operations; core imports no platform API.
pub trait FilePlatform {
    /// Return file identity/revision and observable data availability.
    fn stamp(&self, metadata: &Metadata) -> FileStamp;
    /// Open without following symlinks, materializing cloud data or blocking on FIFOs.
    fn open_regular(&self, path: &Path) -> io::Result<File>;
    /// Read platform resource attributes without resolving aliases or opening data.
    fn resource_metadata(
        &self,
        _path: &Path,
        _metadata: &Metadata,
    ) -> Result<ResourceMetadata, FileError> {
        Ok(ResourceMetadata::unsupported())
    }
}

/// Execute one read-only operation within the explicitly selected root.
pub fn execute(
    platform: &impl FilePlatform,
    request: &FileRequest,
) -> Result<FileResponse, FileError> {
    let scope = scope::Scope::new(platform, &request.root)?;
    let resolved = scope.resolve(
        &request.path,
        request.follow_links,
        matches!(request.action, FileAction::Info | FileAction::Metadata),
    )?;
    let before = scope.info(&resolved)?;
    if !matches!(request.action, FileAction::List(_) | FileAction::Search(_)) {
        check_version(&request.expected_version, &before.version)?;
    }
    let response = dispatch(&scope, &resolved, &before, request)?;
    let after = scope.resolve(
        &request.path,
        request.follow_links,
        matches!(request.action, FileAction::Info | FileAction::Metadata),
    )?;
    if after.path != resolved.path || scope.info(&after)?.version != before.version {
        return Err(FileError::new(
            FileErrorCode::Changed,
            "path or metadata changed during the operation",
        ));
    }
    scope.revalidate_root()?;
    Ok(response)
}

fn dispatch(
    scope: &scope::Scope<'_, impl FilePlatform>,
    resolved: &scope::Resolved,
    before: &FileInfo,
    request: &FileRequest,
) -> Result<FileResponse, FileError> {
    Ok(match &request.action {
        FileAction::Info => FileResponse::Info(before.clone()),
        FileAction::Metadata => {
            FileResponse::Metadata(metadata::inspect(scope, resolved, before.clone())?)
        }
        FileAction::List(options) => FileResponse::List(listing::list(
            scope,
            resolved,
            options,
            &request.expected_version,
        )?),
        FileAction::Read(options) => {
            FileResponse::Read(reading::read(scope, resolved, before.clone(), options)?)
        }
        FileAction::Search(options) => FileResponse::Search(search::search(
            scope,
            resolved,
            options,
            &request.expected_version,
        )?),
    })
}

pub(super) fn check_version(expected: &Option<String>, actual: &str) -> Result<(), FileError> {
    if expected.as_deref().is_some_and(|version| version != actual) {
        return Err(FileError::new(
            FileErrorCode::Changed,
            "expected version no longer matches; observe again",
        ));
    }
    Ok(())
}
