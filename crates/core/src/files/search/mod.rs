//! Metadata-only filesystem search with explicit scope, budgets and query-bound pages.
mod model;
#[cfg(test)]
mod tests;
mod traversal;

pub use model::*;

use super::scope::{Resolved, Scope};
use super::*;
use sha2::{Digest, Sha256};

pub(super) fn search(
    scope: &Scope<'_, impl FilePlatform>,
    directory: &Resolved,
    options: &SearchOptions,
    expected: &Option<String>,
) -> Result<FileSearch, FileError> {
    validate(directory, options, expected)?;
    let directory_info = scope.info(directory)?;
    let mut scan = traversal::scan(scope, directory, options)?;
    scan.entries
        .sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    let version = fingerprint(&directory_info, options, &scan.entries)?;
    super::check_version(expected, &version)?;
    traversal::revalidate(scope, &scan.entries)?;
    let entries_observed = scan.entries.len();
    scan.entries.retain(|entry| matches(entry, options));
    let total = scan.entries.len();
    if options.offset > total {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "offset exceeds search result count",
        ));
    }
    let end = options.offset.saturating_add(options.limit).min(total);
    Ok(FileSearch {
        directory: directory_info,
        source: SearchSource::Filesystem,
        query: options.clone(),
        version,
        entries: scan.entries.drain(options.offset..end).collect(),
        total,
        offset: options.offset,
        next_offset: (end < total).then_some(end),
        enumeration_complete: true,
        entries_observed,
        directories_scanned: scan.directories_scanned,
        depth_boundary_directories: scan.depth_boundary_directories,
    })
}

fn matches(entry: &SearchEntry, options: &SearchOptions) -> bool {
    let info = &entry.file;
    if (!options.hidden && info.name.starts_with('.'))
        || options
            .name
            .as_ref()
            .is_some_and(|name| !info.name.contains(name))
        || options.kind.as_ref().is_some_and(|kind| kind != &info.kind)
        || options.min_size.is_some_and(|min| info.size < min)
        || options.max_size.is_some_and(|max| info.size > max)
    {
        return false;
    }
    if options.modified_after.is_some() || options.modified_before.is_some() {
        let Some(time) = info
            .modified
            .as_deref()
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        else {
            return false;
        };
        if options.modified_after.is_some_and(|min| time < min)
            || options.modified_before.is_some_and(|max| time > max)
        {
            return false;
        }
    }
    true
}

fn validate(
    directory: &Resolved,
    options: &SearchOptions,
    expected: &Option<String>,
) -> Result<(), FileError> {
    if !(1..=500).contains(&options.limit)
        || !(1..=MAX_SEARCH_DEPTH).contains(&options.max_depth)
        || !(1..=MAX_DIRECTORY_ENTRIES).contains(&options.max_entries)
        || (options.offset > 0 && expected.is_none())
        || options.name.as_ref().is_some_and(|name| name.is_empty())
        || options
            .min_size
            .zip(options.max_size)
            .is_some_and(|(min, max)| min > max)
        || options
            .modified_after
            .zip(options.modified_before)
            .is_some_and(|(min, max)| min > max)
    {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "search requires limit 1..500, max-depth 1..32, max-entries 1..10000, nonempty name, ordered ranges and --expected-version for later pages",
        ));
    }
    if !directory.metadata.is_dir() {
        return Err(FileError::new(
            FileErrorCode::UnsupportedType,
            "search requires a directory",
        ));
    }
    Ok(())
}

fn fingerprint(
    directory: &FileInfo,
    options: &SearchOptions,
    entries: &[SearchEntry],
) -> Result<String, FileError> {
    // Bind result-affecting options and budgets, but allow page size to change.
    let mut query = options.clone();
    query.limit = 0;
    query.offset = 0;
    let query = serde_json::to_string(&query)
        .map_err(|error| FileError::new(FileErrorCode::Internal, error.to_string()))?;
    let mut digest = Sha256::new();
    for value in [
        "filesystem-search-v1",
        &directory.path,
        &directory.version,
        &query,
    ]
    .into_iter()
    .chain(
        entries
            .iter()
            .flat_map(|entry| [entry.relative_path.as_str(), entry.file.version.as_str()]),
    ) {
        digest.update((value.len() as u64).to_le_bytes());
        digest.update(value.as_bytes());
    }
    Ok(format!("{:x}", digest.finalize()))
}
