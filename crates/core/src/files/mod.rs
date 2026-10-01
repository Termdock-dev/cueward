//! Bounded, explicitly scoped filesystem reads with platform identity/open hooks.
mod listing;
mod model;
mod reading;
mod scope;

pub use model::*;

use std::fs::{File, Metadata};
use std::io;
use std::path::Path;

/// Platform identity and guarded-open operations; core imports no platform API.
pub trait FilePlatform {
    /// Return file identity/revision and observable data availability.
    fn stamp(&self, metadata: &Metadata) -> FileStamp;
    /// Open without following symlinks, materializing cloud data or blocking on FIFOs.
    fn open_regular(&self, path: &Path) -> io::Result<File>;
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
        matches!(request.action, FileAction::Info),
    )?;
    let before = scope.info(&resolved)?;
    if !matches!(request.action, FileAction::List(_)) {
        check_version(&request.expected_version, &before.version)?;
    }
    let response = match &request.action {
        FileAction::Info => FileResponse::Info(before.clone()),
        FileAction::List(options) => FileResponse::List(listing::list(
            &scope,
            &resolved,
            options,
            &request.expected_version,
        )?),
        FileAction::Read(options) => {
            FileResponse::Read(reading::read(&scope, &resolved, before.clone(), options)?)
        }
    };
    let after = scope.resolve(
        &request.path,
        request.follow_links,
        matches!(request.action, FileAction::Info),
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

pub(super) fn check_version(expected: &Option<String>, actual: &str) -> Result<(), FileError> {
    if expected.as_deref().is_some_and(|version| version != actual) {
        return Err(FileError::new(
            FileErrorCode::Changed,
            "expected version no longer matches; observe again",
        ));
    }
    Ok(())
}
