use super::*;
use cueward_adapter_macos::files::finder::{FinderAction, FinderRequest, execute_worker, run};

#[derive(Subcommand)]
pub(crate) enum FinderCommand {
    /// Observe the front Finder window and selection within root; no activation requested.
    Context {
        #[arg(long)]
        root: PathBuf,
        /// Selection budget; exceeding it returns an error, not a partial selection.
        #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u16).range(1..=500))]
        max_items: u16,
        #[arg(long, default_value_t = 10000, value_parser = clap::value_parser!(u64).range(1..=30000))]
        timeout_ms: u64,
    },
    /// Activate Finder and select one item. Delivery is sent_unverified; observe before retrying.
    Reveal {
        #[arg(long)]
        root: PathBuf,
        /// Explicit path relative to root, without parent (..) components.
        #[arg(long)]
        path: PathBuf,
        #[arg(long)]
        follow_links: bool,
        #[arg(long)]
        expected_version: Option<String>,
        #[arg(long, default_value_t = 10000, value_parser = clap::value_parser!(u64).range(1..=30000))]
        timeout_ms: u64,
    },
}

impl FinderCommand {
    fn request(self) -> (FinderRequest, u64) {
        match self {
            Self::Context {
                root,
                max_items,
                timeout_ms,
            } => (
                FinderRequest {
                    root,
                    action: FinderAction::Context {
                        max_items: max_items.into(),
                    },
                },
                timeout_ms,
            ),
            Self::Reveal {
                root,
                path,
                follow_links,
                expected_version,
                timeout_ms,
            } => (
                FinderRequest {
                    root,
                    action: FinderAction::Reveal {
                        path,
                        follow_links,
                        expected_version,
                    },
                },
                timeout_ms,
            ),
        }
    }
}

/// Run the chosen desktop operation under a parent-controlled deadline.
pub(super) fn dispatch(action: FinderCommand) {
    let (request, timeout) = action.request();
    let result = std::env::current_exe()
        .map_err(FileError::from)
        .and_then(|exe| run(&exe, &request, timeout));
    output("files", result);
}

/// Execute one validated Finder request in the supervised worker.
pub(crate) fn worker() {
    output(
        "files/worker",
        read_request().and_then(|request| execute_worker(&request)),
    );
}

#[cfg(test)]
#[path = "files_finder_tests.rs"]
mod tests;
