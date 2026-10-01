use super::scope::{Resolved, Scope, text};
use super::*;
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::fs;

pub(super) fn list(
    scope: &Scope<'_, impl FilePlatform>,
    directory: &Resolved,
    options: &ListOptions,
    expected: &Option<String>,
) -> Result<FileListing, FileError> {
    validate(directory, options, expected)?;
    let info = scope.info(directory)?;
    let mut entries = enumerate(scope, directory)?;
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    let version = fingerprint(&info, &entries);
    super::check_version(expected, &version)?;
    revalidate(scope, &entries)?;
    entries.retain(|entry| options.hidden || !entry.name.starts_with('.'));
    entries.sort_by(|a, b| compare(a, b, options));
    let total = entries.len();
    if options.offset > total {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "offset exceeds directory result count",
        ));
    }
    let end = options.offset.saturating_add(options.limit).min(total);
    Ok(FileListing {
        directory: info,
        version,
        entries: entries.drain(options.offset..end).collect(),
        total,
        offset: options.offset,
        next_offset: (end < total).then_some(end),
        enumeration_complete: true,
    })
}

fn enumerate(
    scope: &Scope<'_, impl FilePlatform>,
    directory: &Resolved,
) -> Result<Vec<FileInfo>, FileError> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(&directory.path)? {
        if entries.len() == MAX_DIRECTORY_ENTRIES {
            return Err(FileError::new(
                FileErrorCode::ScanLimit,
                "directory exceeds 10000 entries; no partial sorted page is returned",
            ));
        }
        let entry = entry?;
        text(&entry.path())?;
        let metadata = fs::symlink_metadata(entry.path())?;
        let link = metadata.file_type().is_symlink();
        let resolved = Resolved {
            path: entry.path(),
            requested: directory.requested.join(entry.file_name()),
            metadata,
            link,
        };
        entries.push(scope.info(&resolved)?);
    }
    Ok(entries)
}

fn fingerprint(directory: &FileInfo, entries: &[FileInfo]) -> String {
    let mut digest = Sha256::new();
    for value in std::iter::once(directory.version.as_str()).chain(
        entries
            .iter()
            .flat_map(|entry| [entry.name.as_str(), entry.version.as_str()]),
    ) {
        digest.update((value.len() as u64).to_le_bytes());
        digest.update(value.as_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn compare(a: &FileInfo, b: &FileInfo, options: &ListOptions) -> Ordering {
    let primary = match options.sort {
        FileSort::Name => a.name.cmp(&b.name),
        FileSort::Size => a.size.cmp(&b.size),
        FileSort::Modified => {
            let time = |info: &FileInfo| {
                info.modified
                    .as_deref()
                    .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
            };
            time(a).cmp(&time(b))
        }
    };
    let order = primary.then_with(|| a.name.cmp(&b.name));
    if options.descending {
        order.reverse()
    } else {
        order
    }
}

fn validate(
    directory: &Resolved,
    options: &ListOptions,
    expected: &Option<String>,
) -> Result<(), FileError> {
    if options.limit == 0 || options.limit > 500 || (options.offset > 0 && expected.is_none()) {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "limit must be 1..500; pagination requires --expected-version",
        ));
    }
    if !directory.metadata.is_dir() {
        return Err(FileError::new(
            FileErrorCode::UnsupportedType,
            "list requires a directory",
        ));
    }
    Ok(())
}

fn revalidate(scope: &Scope<'_, impl FilePlatform>, entries: &[FileInfo]) -> Result<(), FileError> {
    // Re-observe every child, including hidden/unpaged children used by the token.
    for entry in entries {
        let current = scope.platform.stamp(&fs::symlink_metadata(&entry.path)?);
        if current.version != entry.version {
            return Err(FileError::new(
                FileErrorCode::Changed,
                "directory entry changed while enumerating",
            ));
        }
    }
    Ok(())
}
