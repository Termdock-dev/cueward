use clap::{Args, Subcommand, ValueEnum};
use cueward_core::files::*;
use std::io::Read;
use std::path::PathBuf;

#[path = "files_finder.rs"]
mod finder;
#[path = "files_search.rs"]
mod search;
#[path = "files_spotlight.rs"]
mod spotlight;

#[derive(Args)]
pub(crate) struct ScopeArgs {
    /// Absolute directory to scope this operation to.
    #[arg(long)]
    root: PathBuf,
    /// Path relative to root, without parent (..) components.
    #[arg(long, default_value = ".")]
    path: PathBuf,
    /// Follow symlinks whose resolved targets stay within root.
    #[arg(long)]
    follow_links: bool,
    /// Require the version from an earlier result (required for later list/search pages).
    #[arg(long)]
    expected_version: Option<String>,
    /// Deadline for the filesystem worker, including metadata and directory scans.
    #[arg(long, default_value_t = 10000, value_parser = clap::value_parser!(u64).range(1..=30000))]
    timeout_ms: u64,
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum SortArg {
    Name,
    Size,
    Modified,
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum EncodingArg {
    #[value(name = "utf8")]
    Utf8,
    #[value(name = "utf16-le")]
    Utf16Le,
    #[value(name = "utf16-be")]
    Utf16Be,
    Hex,
}

#[derive(Subcommand)]
pub(crate) enum FilesAction {
    /// Find content-index candidates; index coverage and freshness are unknown.
    Spotlight(spotlight::SpotlightArgs),
    /// Read scoped Finder context, or explicitly activate Finder and reveal an item.
    Finder {
        #[command(subcommand)]
        action: finder::FinderCommand,
    },
    /// List one directory with deterministic sorting and versioned pagination.
    List(ListArgs),
    /// Inspect metadata or a leaf symlink without reading its contents.
    Info {
        #[command(flatten)]
        scope: ScopeArgs,
    },
    /// Read platform content type, Finder tags and package/alias resource attributes.
    Metadata {
        #[command(flatten)]
        scope: ScopeArgs,
    },
    /// Read a bounded byte or UTF-8 line range; use hex for binary data.
    Read(ReadArgs),
    /// Search filenames and metadata within explicit depth and scan bounds.
    Search(search::SearchArgs),
}

#[derive(Args)]
pub(crate) struct ListArgs {
    #[command(flatten)]
    scope: ScopeArgs,
    #[arg(long, default_value_t = 100)]
    limit: usize,
    #[arg(long, default_value_t = 0)]
    offset: usize,
    /// Include names starting with a dot.
    #[arg(long)]
    hidden: bool,
    #[arg(long, value_enum, default_value = "name")]
    sort: SortArg,
    #[arg(long)]
    descending: bool,
}

#[derive(Args)]
pub(crate) struct ReadArgs {
    #[command(flatten)]
    scope: ScopeArgs,
    #[arg(long, conflicts_with = "start_line")]
    offset: Option<u64>,
    #[arg(long, default_value_t = 65536)]
    max_bytes: usize,
    #[arg(long, value_enum, default_value = "utf8")]
    encoding: EncodingArg,
    /// First line, counting from 1. Prefix scanning is limited to 8 MiB.
    #[arg(long)]
    start_line: Option<u64>,
    /// Maximum lines (default 100); max-bytes still applies.
    #[arg(long, requires = "start_line")]
    line_count: Option<usize>,
}

impl FilesAction {
    fn request(self) -> Result<(FileRequest, u64), FileError> {
        let (scope, action) = match self {
            Self::Finder { .. } | Self::Spotlight(_) => {
                return Err(FileError::new(
                    FileErrorCode::InvalidOptions,
                    "Finder and Spotlight use separate native workers",
                ));
            }
            Self::Info { scope } => (scope, FileAction::Info),
            Self::Metadata { scope } => (scope, FileAction::Metadata),
            Self::List(args) => args.action(),
            Self::Read(args) => args.action(),
            Self::Search(args) => args.action(),
        };
        Ok((
            FileRequest {
                root: scope.root,
                path: scope.path,
                follow_links: scope.follow_links,
                expected_version: scope.expected_version,
                action,
            },
            scope.timeout_ms,
        ))
    }
}

impl ListArgs {
    fn action(self) -> (ScopeArgs, FileAction) {
        let sort = match self.sort {
            SortArg::Name => FileSort::Name,
            SortArg::Size => FileSort::Size,
            SortArg::Modified => FileSort::Modified,
        };
        (
            self.scope,
            FileAction::List(ListOptions {
                limit: self.limit,
                offset: self.offset,
                hidden: self.hidden,
                sort,
                descending: self.descending,
            }),
        )
    }
}

impl ReadArgs {
    fn action(self) -> (ScopeArgs, FileAction) {
        let encoding = match self.encoding {
            EncodingArg::Utf8 => FileEncoding::Utf8,
            EncodingArg::Utf16Le => FileEncoding::Utf16Le,
            EncodingArg::Utf16Be => FileEncoding::Utf16Be,
            EncodingArg::Hex => FileEncoding::Hex,
        };
        (
            self.scope,
            FileAction::Read(ReadOptions {
                offset: self.offset.unwrap_or(0),
                max_bytes: self.max_bytes,
                encoding,
                start_line: self.start_line,
                line_count: self.line_count.unwrap_or(100),
            }),
        )
    }
}

/// Dispatch a user command through a deadline-controlled worker.
pub(crate) fn dispatch(action: FilesAction) {
    let action = match action {
        FilesAction::Finder { action } => return finder::dispatch(action),
        FilesAction::Spotlight(args) => return spotlight::dispatch(args),
        action => action,
    };
    let result = action.request().and_then(|(request, timeout)| {
        std::env::current_exe()
            .map_err(FileError::from)
            .and_then(|exe| cueward_adapter_macos::files::run(&exe, &request, timeout))
    });
    output("files", result);
}

/// Read exactly one bounded request in the isolated filesystem worker.
pub(crate) fn worker() {
    use cueward_adapter_macos::files::execute_worker;
    output(
        "files/worker",
        read_request().and_then(|request| execute_worker(&request)),
    );
}

/// Decode one worker request, enforcing the shared size bound before dispatch.
pub(super) fn read_request<T: serde::de::DeserializeOwned>() -> Result<T, FileError> {
    use cueward_adapter_macos::files::MAX_REQUEST_BYTES;
    let mut bytes = Vec::new();
    std::io::stdin()
        .take((MAX_REQUEST_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "request exceeds 16 KiB",
        ));
    }
    serde_json::from_slice(&bytes)
        .map_err(|e| FileError::new(FileErrorCode::InvalidOptions, e.to_string()))
}

fn json<T: serde::Serialize>(result: &Result<T, FileError>) -> Result<String, serde_json::Error> {
    // JSON escaping preserves decoded content while print_external guards delimiters.
    serde_json::to_string_pretty(result).map(|value| value.replace('<', "\\u003c"))
}

/// Emit escaped external JSON with an exit status matching the outer result.
pub(super) fn output<T: serde::Serialize>(source: &str, result: Result<T, FileError>) {
    let success = result.is_ok();
    match json(&result) {
        Ok(payload) => super::helpers::print_external(source, &payload),
        Err(error) => {
            eprintln!("file result serialization failed: {error}");
            std::process::exit(1);
        }
    }
    if !success {
        std::process::exit(1);
    }
}

pub(crate) use finder::worker as finder_worker;
pub(crate) use spotlight::worker as spotlight_worker;

#[cfg(test)]
#[path = "files_tests.rs"]
mod tests;
