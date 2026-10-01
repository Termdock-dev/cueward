use super::*;
use chrono::{DateTime, Utc};

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum KindArg {
    File,
    Directory,
    Symlink,
    Other,
}

#[derive(Args)]
pub(crate) struct SearchArgs {
    #[command(flatten)]
    scope: ScopeArgs,
    /// Case-sensitive literal substring of the basename, without glob expansion.
    #[arg(long)]
    name: Option<String>,
    #[arg(long, value_enum)]
    kind: Option<KindArg>,
    /// Inclusive lower bound on metadata size in bytes.
    #[arg(long)]
    min_size: Option<u64>,
    /// Inclusive upper bound on metadata size in bytes.
    #[arg(long)]
    max_size: Option<u64>,
    /// Inclusive RFC3339 timestamp with timezone (e.g. 2026-10-01T00:00:00+08:00).
    #[arg(long)]
    modified_after: Option<DateTime<Utc>>,
    /// Inclusive RFC3339 timestamp with timezone.
    #[arg(long)]
    modified_before: Option<DateTime<Utc>>,
    /// Include dot names and descend into dot directories within max-depth.
    #[arg(long)]
    hidden: bool,
    /// Child depth to search (1..32); default 1 searches only the starting directory.
    #[arg(long, default_value_t = 1)]
    max_depth: usize,
    /// Maximum observed entries over the entire scan (1..10000), including hidden names.
    #[arg(long, default_value_t = 10000)]
    max_entries: usize,
    #[arg(long, default_value_t = 100)]
    limit: usize,
    #[arg(long, default_value_t = 0)]
    offset: usize,
}

impl SearchArgs {
    pub(super) fn action(self) -> (ScopeArgs, FileAction) {
        let kind = self.kind.map(|kind| match kind {
            KindArg::File => FileKind::File,
            KindArg::Directory => FileKind::Directory,
            KindArg::Symlink => FileKind::Symlink,
            KindArg::Other => FileKind::Other,
        });
        (
            self.scope,
            FileAction::Search(SearchOptions {
                name: self.name,
                kind,
                min_size: self.min_size,
                max_size: self.max_size,
                modified_after: self.modified_after,
                modified_before: self.modified_before,
                hidden: self.hidden,
                max_depth: self.max_depth,
                max_entries: self.max_entries,
                limit: self.limit,
                offset: self.offset,
            }),
        )
    }
}
