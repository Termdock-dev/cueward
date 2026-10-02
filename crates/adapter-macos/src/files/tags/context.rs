use super::super::{MacFiles, observe};
use super::*;
use cueward_core::files::mutation::MutationPlatform;

pub(super) struct Context {
    pub file: File,
    pub before: TagSnapshot,
    pub stored: codec::Stored,
    root: FileInfo,
}
impl Context {
    /// Open an available regular file/directory without symlinks or Finder aliases.
    pub(super) fn prepare(
        root: &Path,
        path: &Path,
        expected: Option<String>,
    ) -> Result<Self, FileError> {
        let root_info = observe(root, Path::new("."), false, None)?;
        let before = observe(root, path, false, expected)?;
        if !matches!(before.kind, FileKind::File | FileKind::Directory)
            || before.data_state != DataState::NotDataless
        {
            return Err(FileError::new(
                FileErrorCode::UnsupportedType,
                "tags require an available regular file/directory; no links/placeholders",
            ));
        }
        let file = if before.kind == FileKind::Directory {
            MacFiles.open_directory(Path::new(&before.path))?
        } else {
            MacFiles.open_regular(Path::new(&before.path))?
        };
        if MacFiles.stamp(&file.metadata()?).version != before.version {
            return Err(changed("opened tag target changed"));
        }
        let resources = MacFiles.resource_metadata(Path::new(&before.path), &file.metadata()?)?;
        if resources.is_alias_file != (ResourceValue::Available { value: false }) {
            return Err(FileError::new(
                FileErrorCode::UnsupportedType,
                "Finder aliases/unknown alias state cannot be tagged",
            ));
        }
        let stored = native::read(&file)?;
        let snapshot = TagSnapshot {
            file: before,
            tags: stored.tags.clone(),
            tags_version: stored.version.clone(),
        };
        let context = Self {
            file,
            before: snapshot,
            stored,
            root: root_info,
        };
        context.revalidate(root, path, true)?;
        verify_native_names(&context.stored, &resources)?;
        Ok(context)
    }
    /// Recheck the selected pathname and held inode, allowing only our post-write revision.
    pub(super) fn revalidate(
        &self,
        root: &Path,
        path: &Path,
        exact: bool,
    ) -> Result<FileInfo, FileError> {
        let root_now = observe(root, Path::new("."), false, None)?;
        if root_now.identity != self.root.identity || root_now.path != self.root.path {
            return Err(changed("tag root moved/replaced"));
        }
        let now = observe(root, path, false, None)?;
        let stamp = MacFiles.stamp(&self.file.metadata()?);
        let before = &self.before.file;
        if now.identity != before.identity
            || now.path != before.path
            || now.version != stamp.version
            || (exact && now.version != before.version)
            || now.size != before.size
            || now.modified != before.modified
            || now.mode != before.mode
            || now.data_state != DataState::NotDataless
        {
            return Err(changed(
                "tag target path/descriptor/content metadata changed",
            ));
        }
        Ok(now)
    }
}

/// Cross-check known descriptor tag state against a fresh Foundation name view.
pub(super) fn verify_native_names(
    stored: &codec::Stored,
    resources: &ResourceMetadata,
) -> Result<(), FileError> {
    if stored.tags.is_empty() && resources.finder_tags == ResourceValue::Unavailable {
        return Ok(());
    }
    let ResourceValue::Available { value } = &resources.finder_tags else {
        return Err(FileError::new(
            FileErrorCode::Unavailable,
            "native Finder tag names are unavailable",
        ));
    };
    let mut actual = value.clone();
    actual.sort();
    let mut expected: Vec<_> = stored.tags.iter().map(|t| t.name.clone()).collect();
    expected.sort();
    if actual != expected {
        return Err(FileError::new(
            FileErrorCode::UnsupportedType,
            "native Finder names disagree with stored representation (legacy labels/unknown format)",
        ));
    }
    Ok(())
}
