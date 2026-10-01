use super::*;
use chrono::{DateTime, Utc};
use std::fs;
use std::path::{Component, Path, PathBuf};

pub(super) struct Scope<'a, P: FilePlatform> {
    pub platform: &'a P,
    pub root: PathBuf,
    original: PathBuf,
    stamp: FileStamp,
}

pub(super) struct Resolved {
    pub path: PathBuf,
    pub requested: PathBuf,
    pub metadata: fs::Metadata,
    pub link: bool,
}

pub(super) fn text(path: &Path) -> Result<String, FileError> {
    path.to_str().map(str::to_owned).ok_or_else(|| {
        FileError::new(
            FileErrorCode::UnsupportedPathEncoding,
            "paths must be valid UTF-8; no lossy filename conversion is performed",
        )
    })
}

impl<'a, P: FilePlatform> Scope<'a, P> {
    /// Validate and observe the explicitly chosen directory root.
    pub fn new(platform: &'a P, root: &Path) -> Result<Self, FileError> {
        if !root.is_absolute() {
            return Err(FileError::new(
                FileErrorCode::InvalidOptions,
                "root must be an absolute directory",
            ));
        }
        text(root)?;
        let resolved = root.canonicalize()?;
        text(&resolved)?;
        let metadata = fs::symlink_metadata(&resolved)?;
        if !metadata.is_dir() {
            return Err(FileError::new(
                FileErrorCode::UnsupportedType,
                "root must be a directory",
            ));
        }
        let stamp = platform.stamp(&metadata);
        if stamp.data_state == DataState::Dataless {
            return Err(FileError::new(
                FileErrorCode::Unavailable,
                "root is dataless; no download is requested",
            ));
        }
        Ok(Self {
            platform,
            root: resolved,
            original: root.to_owned(),
            stamp,
        })
    }

    /// Resolve a relative path using the selected symlink policy.
    pub fn resolve(
        &self,
        path: &Path,
        follow: bool,
        leaf_link: bool,
    ) -> Result<Resolved, FileError> {
        text(path)?;
        let parts: Vec<_> = path
            .components()
            .filter(|c| !matches!(c, Component::CurDir))
            .collect();
        if parts.iter().any(|c| !matches!(c, Component::Normal(_))) {
            return Err(FileError::new(
                FileErrorCode::OutsideRoot,
                "path must be relative to root without parent components",
            ));
        }
        let mut actual = self.root.clone();
        for (index, part) in parts.iter().enumerate() {
            actual.push(part.as_os_str());
            let (next, metadata, link) =
                self.step(actual, follow, leaf_link && index + 1 == parts.len())?;
            actual = next;
            if link {
                return Ok(Resolved {
                    path: actual,
                    requested: self.original.join(path),
                    metadata,
                    link,
                });
            }
            if self.platform.stamp(&metadata).data_state == DataState::Dataless
                && (index + 1 < parts.len() || !leaf_link)
            {
                return Err(FileError::new(
                    FileErrorCode::Unavailable,
                    "path is dataless; no download is requested",
                ));
            }
        }
        let metadata = fs::symlink_metadata(&actual)?;
        Ok(Resolved {
            path: actual,
            requested: self.original.join(path),
            metadata,
            link: false,
        })
    }

    fn step(
        &self,
        mut path: PathBuf,
        follow: bool,
        leaf_link: bool,
    ) -> Result<(PathBuf, fs::Metadata, bool), FileError> {
        let mut metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            if !follow {
                if leaf_link {
                    return Ok((path, metadata, true));
                }
                return Err(FileError::new(
                    FileErrorCode::SymlinkDisallowed,
                    "symlink traversal requires --follow-links",
                ));
            }
            path = path.canonicalize()?;
            if !path.starts_with(&self.root) {
                return Err(FileError::new(
                    FileErrorCode::OutsideRoot,
                    "resolved symlink target is outside root",
                ));
            }
            metadata = fs::symlink_metadata(&path)?;
        }
        Ok((path, metadata, false))
    }

    /// Describe the observed object without opening its data stream.
    pub fn info(&self, resolved: &Resolved) -> Result<FileInfo, FileError> {
        let metadata = &resolved.metadata;
        let stamp = self.platform.stamp(metadata);
        let path = text(&resolved.path)?;
        let name = resolved
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&path)
            .to_owned();
        let link_target = if resolved.link {
            Some(text(&fs::read_link(&resolved.path)?)?)
        } else {
            None
        };
        Ok(FileInfo {
            requested_path: text(&resolved.requested)?,
            path: path.clone(),
            resolved_path: if resolved.link { None } else { Some(path) },
            name,
            kind: kind(resolved),
            identity: stamp.identity,
            version: stamp.version,
            size: metadata.len(),
            modified: timestamp(metadata.modified()),
            created: timestamp(metadata.created()),
            readonly: metadata.permissions().readonly(),
            mode: stamp.mode,
            data_state: stamp.data_state,
            link_target,
        })
    }

    /// Reject root replacement or metadata changes during the operation.
    pub fn revalidate_root(&self) -> Result<(), FileError> {
        let current = self.platform.stamp(&fs::symlink_metadata(&self.root)?);
        if self.original.canonicalize()? != self.root || current != self.stamp {
            return Err(FileError::new(
                FileErrorCode::Changed,
                "root identity or metadata changed; observe again",
            ));
        }
        Ok(())
    }
}

fn kind(resolved: &Resolved) -> FileKind {
    if resolved.link {
        FileKind::Symlink
    } else if resolved.metadata.is_file() {
        FileKind::File
    } else if resolved.metadata.is_dir() {
        FileKind::Directory
    } else {
        FileKind::Other
    }
}

fn timestamp(value: std::io::Result<std::time::SystemTime>) -> Option<String> {
    value
        .ok()
        .map(|time| DateTime::<Utc>::from(time).to_rfc3339())
}
