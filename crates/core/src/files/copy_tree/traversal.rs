use super::*;
use crate::files::scope::text;

pub(super) struct Scan<'a, 'b, P: CopyTreePlatform> {
    scope: &'a Scope<'b, P>,
    request: &'a CopyTreeRequest,
    parent: &'a File,
    pub entries: Vec<CopyTreeEntry>,
    pub bytes: u64,
    pub complete: bool,
    pub different_filesystem: bool,
    // Keep descriptors for all supported nodes until final revision checks.
    json_bytes: usize,
    opened: Vec<(std::path::PathBuf, File, usize)>,
}
impl<'a, 'b, P: CopyTreePlatform> Scan<'a, 'b, P> {
    pub(super) fn new(
        scope: &'a Scope<'b, P>,
        request: &'a CopyTreeRequest,
        parent: &'a File,
    ) -> Self {
        Self {
            scope,
            request,
            parent,
            entries: Vec::new(),
            bytes: 0,
            complete: true,
            different_filesystem: false,
            json_bytes: 0,
            opened: Vec::new(),
        }
    }
    pub(super) fn visit(
        &mut self,
        path: &Path,
        relative: &Path,
        depth: usize,
    ) -> Result<(), FileError> {
        if self.entries.len() >= self.request.max_entries || depth > self.request.max_depth {
            return Err(limit());
        }
        let source = self.scope.info(&self.scope.resolve(path, false, true)?)?;
        if source.kind == FileKind::File {
            self.bytes = self.bytes.checked_add(source.size).ok_or_else(limit)?;
            if self.bytes > self.request.max_bytes {
                return Err(limit());
            }
        }
        let index = self.record(&source, relative, depth)?;
        if self.entries[index].error.is_some() {
            if source.kind == FileKind::Directory {
                self.complete = false;
            }
            return Ok(());
        }
        let file = open(self.scope.platform, &source)?;
        self.different_filesystem |= !self
            .scope
            .platform
            .same_tree_filesystem(&file, self.parent)?;
        if source.kind == FileKind::Directory {
            self.children(&file, path, relative, depth)?;
        }
        recheck(self.scope, path, &source, &file)?;
        self.opened.push((path.to_owned(), file, index));
        Ok(())
    }
    fn record(
        &mut self,
        source: &FileInfo,
        relative: &Path,
        depth: usize,
    ) -> Result<usize, FileError> {
        let (resources, error) = self.inspect(source)?;
        let index = self.entries.len();
        self.entries.push(CopyTreeEntry {
            relative_path: relative.to_owned(),
            destination: if depth == 0 {
                self.request.destination.clone()
            } else {
                self.request.destination.join(relative)
            },
            source: source.clone(),
            resources,
            error,
        });
        self.json_bytes += serde_json::to_vec_pretty(&self.entries[index])
            .map_err(|e| FileError::new(FileErrorCode::Internal, e.to_string()))?
            .len();
        if self.json_bytes > MAX_TREE_PLAN_BYTES {
            return Err(limit());
        }
        Ok(index)
    }
    fn children(
        &mut self,
        file: &File,
        path: &Path,
        relative: &Path,
        depth: usize,
    ) -> Result<(), FileError> {
        let mut names = self
            .scope
            .platform
            .tree_names(file, self.request.max_entries - self.entries.len())?;
        names.sort();
        for name in names {
            text(Path::new(&name))?;
            let child = if relative == Path::new(".") {
                std::path::PathBuf::from(&name)
            } else {
                relative.join(&name)
            };
            self.visit(&path.join(&name), &child, depth + 1)?;
        }
        Ok(())
    }
    fn inspect(
        &self,
        source: &FileInfo,
    ) -> Result<(Option<ResourceMetadata>, Option<FileError>), FileError> {
        if !matches!(source.kind, FileKind::File | FileKind::Directory)
            || source.data_state != DataState::NotDataless
        {
            return Ok((
                None,
                Some(unsupported(
                    "links, special files and unavailable data are not planned for copying",
                )),
            ));
        }
        let file = open(self.scope.platform, source)?;
        let resources = self
            .scope
            .platform
            .resource_metadata(Path::new(&source.path), &file.metadata()?)?;
        let error = if resources.is_alias_file != (ResourceValue::Available { value: false }) {
            Some(unsupported(
                "Finder aliases or unknown alias state cannot be copied",
            ))
        } else if source.kind == FileKind::Directory
            && resources.is_package != (ResourceValue::Available { value: false })
        {
            Some(unsupported(
                "packages or unknown package state are not traversed",
            ))
        } else {
            self.scope
                .platform
                .validate_tree_metadata(&file, source)
                .err()
        };
        Ok((Some(resources), error))
    }
    pub(super) fn revalidate(&self) -> Result<(), FileError> {
        for (path, file, index) in &self.opened {
            recheck(self.scope, path, &self.entries[*index].source, file)?;
        }
        // Unsupported observations still have to remain bound to the original revision.
        for entry in &self.entries {
            let path = Path::new(&entry.source.path)
                .strip_prefix(&self.scope.root)
                .map_err(|_| changed())?;
            let now = self.scope.info(&self.scope.resolve(path, false, true)?)?;
            if now.path != entry.source.path || now.version != entry.source.version {
                return Err(changed());
            }
        }
        Ok(())
    }
}

pub(super) fn open(platform: &impl CopyTreePlatform, info: &FileInfo) -> Result<File, FileError> {
    if info.data_state != DataState::NotDataless {
        return Err(unsupported(
            "directory/file availability is unknown or dataless",
        ));
    }
    let file = match info.kind {
        FileKind::File => platform.open_regular(Path::new(&info.path))?,
        FileKind::Directory => platform.open_tree_directory(Path::new(&info.path))?,
        _ => {
            return Err(unsupported(
                "copy-tree requires a regular file or directory",
            ));
        }
    };
    if platform.stamp(&file.metadata()?).version != info.version {
        return Err(changed());
    }
    Ok(file)
}
fn unsupported(message: &str) -> FileError {
    FileError::new(FileErrorCode::UnsupportedType, message)
}
fn limit() -> FileError {
    FileError::new(
        FileErrorCode::ScanLimit,
        "copy-tree exceeds entries/depth/observed file bytes budget; no partial plan returned",
    )
}
