//! Bounded Spotlight content-index candidates with conservative scope checks.
mod model;
mod native;
#[cfg(test)]
mod tests;

use super::{observe, policy, protocol};
use cueward_core::files::*;
pub use model::*;
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

trait Native {
    fn search(
        &self,
        root: &Path,
        query: &str,
        max_candidates: usize,
    ) -> Result<Vec<PathBuf>, FileError>;
}

struct System;
impl Native for System {
    fn search(
        &self,
        root: &Path,
        query: &str,
        max_candidates: usize,
    ) -> Result<Vec<PathBuf>, FileError> {
        native::search(root, query, max_candidates)
    }
}

/// Execute the isolated index worker with bounded input/output and a hard deadline.
pub fn run(
    executable: &Path,
    request: &SpotlightRequest,
    timeout_ms: u64,
) -> Result<SpotlightResponse, FileError> {
    protocol::run_typed(executable, "files-spotlight-worker", request, timeout_ms)
}

/// Execute only inside the supervised worker; do not request imports or downloads.
pub fn execute_worker(request: &SpotlightRequest) -> Result<SpotlightResponse, FileError> {
    let _policy = policy::NoMaterialization::enter()?;
    execute(request, &System)
}

fn execute(
    request: &SpotlightRequest,
    native: &impl Native,
) -> Result<SpotlightResponse, FileError> {
    let query = expression(request)?;
    let root = observe(&request.root, Path::new("."), false, None)?;
    let candidates = native.search(Path::new(&root.path), &query, request.max_candidates)?;
    if candidates.len() > request.max_candidates {
        return Err(FileError::new(
            FileErrorCode::ScanLimit,
            "Spotlight candidates exceed max-candidates; narrow root or text",
        ));
    }
    let mut path_bytes = 0;
    for candidate in &candidates {
        budget_path(&mut path_bytes, candidate)?;
    }
    let count = candidates.len();
    let (mut entries, excluded) = collect(request, &root, candidates)?;
    entries.sort_by(|a, b| a.relative_path().cmp(b.relative_path()));
    let after = observe(
        &request.root,
        Path::new("."),
        false,
        Some(root.version.clone()),
    )?;
    if after.path != root.path {
        return Err(FileError::new(
            FileErrorCode::Changed,
            "root path changed during Spotlight query",
        ));
    }
    bounded_response(response(request, root, count, entries, excluded))
}

fn bounded_response(response: SpotlightResponse) -> Result<SpotlightResponse, FileError> {
    let bytes = serde_json::to_vec(&response)
        .map_err(|e| FileError::new(FileErrorCode::Internal, e.to_string()))?;
    let escaped_size = bytes.len() + 5 * bytes.iter().filter(|b| **b == b'<').count();
    if escaped_size > 8 * 1024 * 1024 {
        return Err(FileError::new(
            FileErrorCode::ScanLimit,
            "Spotlight result exceeds 8 MiB; narrow root/text or reduce limit",
        ));
    }
    Ok(response)
}

fn response(
    request: &SpotlightRequest,
    root: FileInfo,
    count: usize,
    mut entries: Vec<SpotlightEntry>,
    excluded: ExcludedCandidates,
) -> SpotlightResponse {
    let available_count = entries
        .iter()
        .filter(|e| matches!(e, SpotlightEntry::Available { .. }))
        .count();
    let eligible_count = entries.len();
    let error_count = eligible_count - available_count;
    let truncated = eligible_count > request.limit;
    entries.truncate(request.limit);
    SpotlightResponse::Spotlight(SpotlightResult {
        root,
        source: SpotlightSource::Spotlight,
        query: request.clone(),
        query_completed: true,
        index_coverage: IndexCoverage::Unknown,
        enumeration_complete: false,
        content_verified: false,
        candidate_count: count,
        eligible_count,
        available_count,
        error_count,
        excluded,
        entries,
        truncated,
    })
}

fn expression(request: &SpotlightRequest) -> Result<String, FileError> {
    if !(1..=32).contains(&request.max_depth)
        || !(1..=10000).contains(&request.max_candidates)
        || !(1..=500).contains(&request.limit)
    {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "max-depth must be 1..32, max-candidates 1..10000 and limit 1..500",
        ));
    }
    let text = &request.text;
    if text.trim().is_empty()
        || text.len() > 1024
        || text
            .chars()
            .any(|c| c.is_control() || matches!(c, '*' | '?'))
    {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "text must be 1..1024 UTF-8 bytes, contain non-whitespace, and exclude controls and * or ?; raw predicates are not accepted",
        ));
    }
    // Spotlight uses glob matching. Quotes/backslashes stay inside one value;
    // user wildcard characters are rejected rather than given ambiguous escapes.
    let escaped = text.replace('\\', "\\\\").replace('"', "\\\"");
    Ok(format!("kMDItemTextContent == \"*{escaped}*\"c"))
}

fn collect(
    request: &SpotlightRequest,
    root: &FileInfo,
    candidates: Vec<PathBuf>,
) -> Result<(Vec<SpotlightEntry>, ExcludedCandidates), FileError> {
    let mut excluded = ExcludedCandidates::default();
    let mut seen = BTreeSet::new();
    let mut entries = Vec::new();
    for candidate in candidates {
        let Some(relative) = relative(&candidate, root, request, &mut excluded)? else {
            continue;
        };
        if !seen.insert(relative.clone()) {
            excluded.duplicate += 1;
            continue;
        }
        let observed = observe(&request.root, Path::new(&relative), false, None);
        match observed {
            Ok(file) if file.kind == FileKind::File => entries.push(SpotlightEntry::Available {
                relative_path: relative,
                file: Box::new(file),
            }),
            Ok(_) => excluded.non_regular += 1,
            Err(error) => entries.push(SpotlightEntry::Error {
                relative_path: relative,
                error,
            }),
        }
    }
    Ok((entries, excluded))
}

fn relative(
    path: &Path,
    root: &FileInfo,
    request: &SpotlightRequest,
    excluded: &mut ExcludedCandidates,
) -> Result<Option<String>, FileError> {
    let Some(relative) = path
        .strip_prefix(&root.path)
        .ok()
        .filter(|p| !p.as_os_str().is_empty())
    else {
        excluded.outside_scope += 1;
        return Ok(None);
    };
    let parts: Vec<_> = relative.components().collect();
    if parts.iter().any(|c| !matches!(c, Component::Normal(_))) {
        excluded.outside_scope += 1;
        return Ok(None);
    }
    let text = relative.to_str().ok_or_else(|| {
        FileError::new(
            FileErrorCode::UnsupportedPathEncoding,
            "Spotlight path is not UTF-8",
        )
    })?;
    if text.len() > 16384 {
        return Err(FileError::new(
            FileErrorCode::ScanLimit,
            "Spotlight path exceeds 16 KiB",
        ));
    }
    if parts.len() > request.max_depth {
        excluded.depth += 1;
        return Ok(None);
    }
    if !request.hidden
        && parts
            .iter()
            .any(|c| c.as_os_str().to_str().is_some_and(|s| s.starts_with('.')))
    {
        excluded.hidden += 1;
        return Ok(None);
    }
    Ok(Some(text.to_owned()))
}

// Bound all candidate paths before filtering, including duplicates/outside paths.
fn budget_path(total: &mut usize, path: &Path) -> Result<(), FileError> {
    let text = path.to_str().ok_or_else(|| {
        FileError::new(
            FileErrorCode::UnsupportedPathEncoding,
            "Spotlight path is not UTF-8",
        )
    })?;
    if text.contains('\0') {
        return Err(FileError::new(
            FileErrorCode::UnsupportedPathEncoding,
            "Spotlight path contains NUL",
        ));
    }
    *total += text.len();
    if text.len() > 16384 || *total > 4 * 1024 * 1024 {
        return Err(FileError::new(
            FileErrorCode::ScanLimit,
            "Spotlight paths exceed 16 KiB each or 4 MiB total",
        ));
    }
    Ok(())
}
