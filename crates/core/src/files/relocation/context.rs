use super::*;
use crate::files::scope::{Scope, text};
use std::fs;

pub(super) struct Observed {
    pub file: File,
    pub info: FileInfo,
}
pub(super) struct Context<'a, P: RelocationPlatform> {
    scope: Scope<'a, P>,
    root: File,
    pub root_info: FileInfo,
    pub source: Observed,
    pub source_parent: Observed,
    pub destination_parent: Observed,
}

impl<'a, P: RelocationPlatform> Context<'a, P> {
    /// Observe and hold the root/source/parents without changing their namespace.
    pub(super) fn prepare(platform: &'a P, request: &RelocationRequest) -> Result<Self, FileError> {
        let scope = Scope::new(platform, &request.root)?;
        let root_info = scope.info(&scope.resolve(Path::new("."), false, false)?)?;
        let root = platform.open_directory(&scope.root)?;
        check_descriptor(platform, &root, &root_info)?;
        let source = observe(&scope, &request.path, false)?;
        check_version(
            &Some(request.expected_version.clone()),
            &source.info.version,
        )?;
        let source_parent = observe(&scope, validation::parent(&request.path), true)?;
        let destination_parent = observe(&scope, validation::parent(&request.destination), true)?;
        check_version(
            &Some(request.expected_parent_version.clone()),
            &destination_parent.info.version,
        )?;
        if source.info.kind == FileKind::Directory {
            reject_descendant(&scope, &source.info, &destination_parent.info)?;
        }
        Ok(Self {
            scope,
            root,
            root_info,
            source,
            source_parent,
            destination_parent,
        })
    }

    /// Return the proposed absolute path, not proof of a delivered object.
    pub(super) fn destination_path(
        &self,
        request: &RelocationRequest,
    ) -> Result<String, FileError> {
        let leaf = request
            .destination
            .file_name()
            .ok_or_else(|| changed("missing destination leaf"))?;
        text(&Path::new(&self.destination_parent.info.path).join(leaf))
    }

    /// Record conflicts including a leaf link, without following its target.
    pub(super) fn destination(
        &self,
        request: &RelocationRequest,
    ) -> Result<Option<FileInfo>, FileError> {
        match fs::symlink_metadata(self.destination_path(request)?) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
            Ok(_) => Ok(Some(self.scope.info(&self.scope.resolve(
                &request.destination,
                false,
                true,
            )?)?)),
        }
    }

    /// Query native source metadata and reject aliases or unknown alias state.
    pub(super) fn resources(&self, platform: &P) -> Result<ResourceMetadata, FileError> {
        let resources = platform.resource_metadata(
            Path::new(&self.source.info.path),
            &self.source.file.metadata()?,
        )?;
        match resources.is_alias_file {
            ResourceValue::Available { value: false } => Ok(resources),
            ResourceValue::Available { value: true } => Err(FileError::new(
                FileErrorCode::UnsupportedType,
                "Finder aliases are not planned as relocation targets",
            )),
            _ => Err(FileError::new(
                FileErrorCode::Unavailable,
                "cannot establish whether source is a Finder alias",
            )),
        }
    }

    /// Reject observed descriptor/path revisions that changed during planning.
    pub(super) fn revalidate(&self, request: &RelocationRequest) -> Result<(), FileError> {
        for (path, observed) in [
            (request.path.as_path(), &self.source),
            (validation::parent(&request.path), &self.source_parent),
            (
                validation::parent(&request.destination),
                &self.destination_parent,
            ),
        ] {
            check_descriptor(self.scope.platform, &observed.file, &observed.info)?;
            let current = self.scope.info(&self.scope.resolve(path, false, false)?)?;
            if current.path != observed.info.path || current.version != observed.info.version {
                return Err(changed("source or parent path changed while planning"));
            }
        }
        check_descriptor(self.scope.platform, &self.root, &self.root_info)?;
        self.scope.revalidate_root()
    }
}

fn observe<P: RelocationPlatform>(
    scope: &Scope<'_, P>,
    path: &Path,
    directory: bool,
) -> Result<Observed, FileError> {
    let resolved = scope.resolve(path, false, false)?;
    let info = scope.info(&resolved)?;
    if info.data_state != DataState::NotDataless {
        return Err(FileError::new(
            FileErrorCode::Unavailable,
            "planning never materializes unavailable/unknown source data",
        ));
    }
    if !matches!(info.kind, FileKind::File | FileKind::Directory)
        || (directory && info.kind != FileKind::Directory)
    {
        return Err(FileError::new(
            FileErrorCode::UnsupportedType,
            "planning requires available regular files or directories",
        ));
    }
    let file = match info.kind {
        FileKind::Directory => scope.platform.open_directory(&resolved.path)?,
        _ => scope.platform.open_regular(&resolved.path)?,
    };
    check_descriptor(scope.platform, &file, &info)?;
    Ok(Observed { file, info })
}

fn check_descriptor(
    platform: &impl FilePlatform,
    file: &File,
    info: &FileInfo,
) -> Result<(), FileError> {
    if platform.stamp(&file.metadata()?).version != info.version {
        return Err(changed(
            "opened source/parent revision changed while planning",
        ));
    }
    Ok(())
}

fn reject_descendant<P: RelocationPlatform>(
    scope: &Scope<'_, P>,
    source: &FileInfo,
    destination_parent: &FileInfo,
) -> Result<(), FileError> {
    let mut path = Path::new(&destination_parent.path);
    loop {
        if scope.platform.stamp(&fs::symlink_metadata(path)?).identity == source.identity {
            return Err(FileError::new(
                FileErrorCode::InvalidOptions,
                "a directory cannot be moved into itself or a descendant",
            ));
        }
        if path == scope.root {
            return Ok(());
        }
        path = path
            .parent()
            .filter(|p| p.starts_with(&scope.root))
            .ok_or_else(|| changed("destination parent escaped root"))?;
    }
}
fn changed(message: &str) -> FileError {
    FileError::new(FileErrorCode::Changed, message)
}
