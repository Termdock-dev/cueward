use super::*;
use crate::files::scope::{Resolved, Scope, text};
use std::fs;
use std::path::{Component, PathBuf};

pub(super) struct Source {
    pub file: File,
    pub info: FileInfo,
}
pub(super) struct Context<'a, P: MutationPlatform> {
    pub scope: Scope<'a, P>,
    pub root: File,
    pub parent: File,
    pub parent_info: FileInfo,
    pub path: PathBuf,
    pub source: Option<Source>,
}

pub(super) fn prepare<'a, P: MutationPlatform>(
    platform: &'a P,
    receipt: &mut MutationReceipt,
) -> Result<Context<'a, P>, FileError> {
    validate(&receipt.request)?;
    let scope = Scope::new(platform, &receipt.request.root)?;
    receipt.root_before = Some(scope.info(&scope.resolve(Path::new("."), false, false)?)?);
    let root = platform.open_directory(&scope.root)?;
    scope.revalidate_root()?;
    if platform.stamp(&root.metadata()?).version
        != receipt
            .root_before
            .as_ref()
            .ok_or_else(|| changed("missing root observation"))?
            .version
    {
        return Err(changed("opened root changed"));
    }
    let destination = prepare_destination(&scope, receipt)?;
    let source = source(&scope, &receipt.request.action)?;
    receipt.source_before = source.as_ref().map(|s| s.info.clone());
    Ok(Context {
        scope,
        root,
        parent: destination.parent,
        parent_info: destination.info,
        path: destination.path,
        source,
    })
}

struct Destination {
    parent: File,
    info: FileInfo,
    path: PathBuf,
}
fn parent_path(request: &MutationRequest) -> &Path {
    request
        .destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}
fn prepare_destination<P: MutationPlatform>(
    scope: &Scope<'_, P>,
    receipt: &mut MutationReceipt,
) -> Result<Destination, FileError> {
    let request = &receipt.request;
    let resolved = scope.resolve(parent_path(request), false, false)?;
    let info = scope.info(&resolved)?;
    check_version(
        &Some(request.expected_parent_version.clone()),
        &info.version,
    )?;
    if info.kind != FileKind::Directory || info.data_state != DataState::NotDataless {
        return Err(FileError::new(
            FileErrorCode::UnsupportedType,
            "destination parent must be an available directory",
        ));
    }
    receipt.parent_before = Some(info.clone());
    let parent = scope.platform.open_directory(&resolved.path)?;
    if !parent.metadata()?.is_dir()
        || scope.platform.stamp(&parent.metadata()?).version != info.version
    {
        return Err(changed("opened destination parent changed"));
    }
    let name = request
        .destination
        .file_name()
        .ok_or_else(|| changed("missing destination name"))?
        .to_owned();
    let path = resolved.path.join(&name);
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
        Ok(_) => {
            receipt.destination_before =
                Some(scope.info(&scope.resolve(&request.destination, false, true)?)?);
            return Err(FileError::new(
                FileErrorCode::Conflict,
                "destination already exists; nothing overwritten",
            ));
        }
    }
    Ok(Destination { parent, info, path })
}

fn validate(request: &MutationRequest) -> Result<(), FileError> {
    super::duplication::validate_sibling(request)?;
    let parts: Vec<_> = request.destination.components().collect();
    if parts.is_empty()
        || parts.iter().any(|p| !matches!(p, Component::Normal(_)))
        || request.expected_parent_version.is_empty()
    {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "destination must contain only relative normal components; expected-parent-version is required",
        ));
    }
    text(&request.destination)?;
    if let MutationAction::Copy {
        expected_version,
        max_bytes,
        ..
    }
    | MutationAction::Duplicate {
        expected_version,
        max_bytes,
        ..
    } = &request.action
    {
        if expected_version.is_empty() || !(1..=256 * 1024 * 1024).contains(max_bytes) {
            return Err(FileError::new(
                FileErrorCode::InvalidOptions,
                "copy/duplicate require expected-version and max-bytes 1..268435456",
            ));
        }
    }
    Ok(())
}

fn source<P: MutationPlatform>(
    scope: &Scope<'_, P>,
    action: &MutationAction,
) -> Result<Option<Source>, FileError> {
    let (MutationAction::Copy {
        path,
        expected_version,
        max_bytes,
    }
    | MutationAction::Duplicate {
        path,
        expected_version,
        max_bytes,
    }) = action
    else {
        return Ok(None);
    };
    let resolved = scope.resolve(path, false, true)?;
    let info = scope.info(&resolved)?;
    check_version(&Some(expected_version.clone()), &info.version)?;
    validate_source_info(&info, *max_bytes)?;
    let file = scope.platform.open_regular(&resolved.path)?;
    check_source(scope.platform, &file, &info)?;
    scope.platform.validate_copy_source(&file, &info)?;
    check_source(scope.platform, &file, &info)?;
    Ok(Some(Source { file, info }))
}

fn validate_source_info(info: &FileInfo, maximum: u64) -> Result<(), FileError> {
    if info.kind != FileKind::File {
        return Err(FileError::new(
            FileErrorCode::UnsupportedType,
            "copy/duplicate accept one regular file, not directories/packages/links",
        ));
    }
    if info.data_state != DataState::NotDataless {
        return Err(FileError::new(
            FileErrorCode::Unavailable,
            "copy/duplicate never download placeholder data",
        ));
    }
    if info.size > maximum {
        return Err(FileError::new(
            FileErrorCode::ScanLimit,
            "source exceeds max-bytes",
        ));
    }
    Ok(())
}

pub(super) fn check_source(
    platform: &impl FilePlatform,
    file: &File,
    info: &FileInfo,
) -> Result<(), FileError> {
    let metadata = file.metadata()?;
    let stamp = platform.stamp(&metadata);
    if !metadata.is_file()
        || stamp.version != info.version
        || stamp.data_state != DataState::NotDataless
    {
        return Err(changed("source descriptor changed"));
    }
    Ok(())
}

impl<P: MutationPlatform> Context<'_, P> {
    pub(super) fn revalidate_before(&self, receipt: &MutationReceipt) -> Result<(), FileError> {
        self.scope.revalidate_root()?;
        let resolved = self
            .scope
            .resolve(parent_path(&receipt.request), false, false)?;
        if resolved.path != Path::new(&self.parent_info.path)
            || self.scope.info(&resolved)?.version != self.parent_info.version
            || self.scope.platform.stamp(&self.parent.metadata()?).version
                != self.parent_info.version
        {
            return Err(changed("destination parent changed before mutation"));
        }
        if let Some(source) = &self.source {
            self.check_source_path(source)?;
        }
        Ok(())
    }

    pub(super) fn destination_info(&self, file: &File) -> Result<FileInfo, FileError> {
        self.scope.info(&Resolved {
            path: self.path.clone(),
            requested: self.path.clone(),
            metadata: file.metadata()?,
            link: false,
        })
    }

    pub(super) fn finish(
        &self,
        file: &File,
        receipt: &mut MutationReceipt,
    ) -> Result<(), FileError> {
        let root = self.post_root(receipt)?;
        let parent = self.post_parent(&root, &receipt.request)?;
        receipt.parent_after = Some(parent.clone());
        let destination = self.post_destination(&root, &receipt.request, file)?;
        if let Some(source) = &self.source {
            receipt.source_after = Some(self.check_source_path(source)?);
            let reopened = self.scope.platform.open_regular(&self.path)?;
            copying::verify_readable(
                self.scope.platform,
                &reopened,
                &destination,
                receipt
                    .verification
                    .as_ref()
                    .ok_or_else(|| changed("missing copy verification"))?,
            )?;
        } else if destination.kind != FileKind::Directory
            || fs::read_dir(&self.path)?.next().is_some()
        {
            return Err(changed(
                "created directory is not empty or is no longer a directory",
            ));
        }
        if self.scope.platform.stamp(&file.metadata()?).version != destination.version {
            return Err(changed("destination changed during final verification"));
        }
        // Read-back descriptors survive parent relocation. Re-resolve both paths only
        // after verification, and compare against the observations used for read-back.
        let final_parent = self.post_parent(&root, &receipt.request)?;
        let final_destination = self.post_destination(&root, &receipt.request, file)?;
        if final_parent.version != parent.version
            || final_destination.version != destination.version
        {
            return Err(changed(
                "destination parent or path changed during final verification",
            ));
        }
        root.revalidate_root()?;
        receipt.parent_after = Some(final_parent);
        receipt.destination_after = Some(final_destination);
        Ok(())
    }

    fn post_root(&self, receipt: &mut MutationReceipt) -> Result<Scope<'_, P>, FileError> {
        let root = Scope::new(self.scope.platform, &receipt.request.root)?;
        let info = root.info(&root.resolve(Path::new("."), false, false)?)?;
        let before = receipt
            .root_before
            .as_ref()
            .ok_or_else(|| changed("missing root observation"))?;
        if root.root != self.scope.root || info.identity != before.identity {
            return Err(changed("root moved or replaced"));
        }
        receipt.root_after = Some(info);
        Ok(root)
    }
    fn post_parent(
        &self,
        root: &Scope<'_, P>,
        request: &MutationRequest,
    ) -> Result<FileInfo, FileError> {
        let parent = root.info(&root.resolve(parent_path(request), false, false)?)?;
        if parent.path != self.parent_info.path
            || parent.identity != self.parent_info.identity
            || self.scope.platform.stamp(&self.parent.metadata()?).version != parent.version
        {
            return Err(changed("destination parent moved, replaced or changed"));
        }
        Ok(parent)
    }
    fn post_destination(
        &self,
        root: &Scope<'_, P>,
        request: &MutationRequest,
        file: &File,
    ) -> Result<FileInfo, FileError> {
        let destination = root.info(&root.resolve(&request.destination, false, false)?)?;
        if destination.path != text(&self.path)?
            || destination.version != self.scope.platform.stamp(&file.metadata()?).version
        {
            return Err(changed("created destination path changed"));
        }
        Ok(destination)
    }

    fn check_source_path(&self, source: &Source) -> Result<FileInfo, FileError> {
        check_source(self.scope.platform, &source.file, &source.info)?;
        let path = Path::new(&source.info.path)
            .strip_prefix(&self.scope.root)
            .map_err(|_| changed("source escaped root"))?;
        let after = self.scope.info(&self.scope.resolve(path, false, false)?)?;
        if after.path != source.info.path || after.version != source.info.version {
            return Err(changed("source path changed"));
        }
        Ok(after)
    }
}

pub(super) fn changed(message: &str) -> FileError {
    FileError::new(FileErrorCode::Changed, message)
}
