use super::*;
use cueward_core::files::mutation::{MutationAction, MutationRequest, duplicate_destination};

#[derive(Args)]
pub(crate) struct DuplicateArgs {
    #[arg(long)]
    root: PathBuf,
    /// Source file, directory/package or symlink object relative to root; links are not followed.
    #[arg(long)]
    path: PathBuf,
    /// One exact new filename in the source's existing parent; no automatic conflict naming.
    #[arg(long)]
    name: String,
    #[arg(long)]
    expected_version: String,
    /// Version from files info on the source's existing parent directory.
    #[arg(long)]
    expected_parent_version: String,
    #[arg(long, default_value_t = 67108864, value_parser=clap::value_parser!(u64).range(1..=268435456))]
    max_bytes: u64,
    #[arg(long, default_value_t = 10000, value_parser=clap::value_parser!(u64).range(1..=30000))]
    timeout_ms: u64,
}

/// Derive one sibling destination and reuse the supervised copy/publication engine.
pub(super) fn dispatch(args: DuplicateArgs) {
    match duplicate_destination(&args.path, std::path::Path::new(&args.name)) {
        Ok(destination) => mutation::dispatch(
            MutationRequest {
                root: args.root,
                destination,
                expected_parent_version: args.expected_parent_version,
                action: MutationAction::Duplicate {
                    path: args.path,
                    expected_version: args.expected_version,
                    max_bytes: args.max_bytes,
                },
            },
            args.timeout_ms,
        ),
        Err(error) => output::<()>("files", Err(error)),
    }
}

#[cfg(test)]
#[path = "files_duplicate_tests.rs"]
mod tests;
