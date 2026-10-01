use super::*;
use crate::files::scope::text;
use std::fs;
use std::path::Path;

pub(super) struct Scan {
    pub entries: Vec<SearchEntry>,
    pub directories_scanned: usize,
    pub depth_boundary_directories: usize,
}

pub(super) fn scan(
    scope: &Scope<'_, impl FilePlatform>,
    directory: &Resolved,
    options: &SearchOptions,
) -> Result<Scan, FileError> {
    let mut scan = Scan {
        entries: Vec::new(),
        directories_scanned: 0,
        depth_boundary_directories: 0,
    };
    let mut pending = vec![(scope.info(directory)?, 0)];
    while let Some((parent, depth)) = pending.pop() {
        let resolved = resolve_directory(scope, &parent)?;
        scan.directories_scanned += 1;
        for child in fs::read_dir(&resolved.path)? {
            if scan.entries.len() == options.max_entries {
                return Err(FileError::new(
                    FileErrorCode::ScanLimit,
                    "search exceeds max-entries; no partial sorted page is returned",
                ));
            }
            let entry = observe_child(scope, &parent, child?)?;
            let info = &entry.file;
            let visible = options.hidden || !info.name.starts_with('.');
            if visible && info.kind == FileKind::Directory {
                if depth + 1 < options.max_depth {
                    if info.data_state == DataState::Dataless {
                        return Err(FileError::new(
                            FileErrorCode::Unavailable,
                            "search would traverse a dataless directory; no download is requested",
                        ));
                    }
                    pending.push((info.clone(), depth + 1));
                } else {
                    scan.depth_boundary_directories += 1;
                }
            }
            scan.entries.push(entry);
        }
    }
    Ok(scan)
}

fn resolve_directory(
    scope: &Scope<'_, impl FilePlatform>,
    parent: &FileInfo,
) -> Result<Resolved, FileError> {
    let relative = Path::new(&parent.path)
        .strip_prefix(&scope.root)
        .map_err(|_| FileError::new(FileErrorCode::OutsideRoot, "search directory escaped root"))?;
    let resolved = scope.resolve(relative, false, false)?;
    if resolved.path != Path::new(&parent.path)
        || scope.platform.stamp(&resolved.metadata).version != parent.version
    {
        return Err(FileError::new(
            FileErrorCode::Changed,
            "directory changed before traversal",
        ));
    }
    Ok(resolved)
}

fn observe_child(
    scope: &Scope<'_, impl FilePlatform>,
    parent: &FileInfo,
    child: fs::DirEntry,
) -> Result<SearchEntry, FileError> {
    let relative_path = child
        .path()
        .strip_prefix(&scope.root)
        .map_err(|_| FileError::new(FileErrorCode::OutsideRoot, "search entry escaped root"))?
        .to_owned();
    let mut resolved = scope.resolve(&relative_path, false, true)?;
    resolved.requested = Path::new(&parent.requested_path).join(child.file_name());
    Ok(SearchEntry {
        relative_path: text(&relative_path)?,
        file: scope.info(&resolved)?,
    })
}

pub(super) fn revalidate(
    scope: &Scope<'_, impl FilePlatform>,
    entries: &[SearchEntry],
) -> Result<(), FileError> {
    // Re-observe nonmatching and hidden entries too; they affect result completeness.
    for entry in entries {
        let current = scope.resolve(Path::new(&entry.relative_path), false, true)?;
        if current.path != Path::new(&entry.file.path)
            || scope.platform.stamp(&current.metadata).version != entry.file.version
        {
            return Err(FileError::new(
                FileErrorCode::Changed,
                "search entry changed while scanning",
            ));
        }
    }
    Ok(())
}
