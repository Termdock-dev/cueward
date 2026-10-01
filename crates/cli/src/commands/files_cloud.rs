use super::*;
use cueward_adapter_macos::files::cloud::{CloudAction, CloudRequest, execute_worker, run};

#[derive(Subcommand)]
pub(crate) enum CloudCommand {
    /// Observe per-field iCloud state; third-party provider state remains unknown.
    Status {
        #[command(flatten)]
        scope: ScopeArgs,
    },
    /// Submit one explicit asynchronous iCloud file download; completion is unverified.
    Download {
        #[arg(long)]
        root: PathBuf,
        /// Explicit file path relative to root. Directory downloads are unsupported.
        #[arg(long)]
        path: PathBuf,
        /// Revision obtained from a fresh cloud status/info observation.
        #[arg(long)]
        expected_version: String,
        #[arg(long)]
        follow_links: bool,
        /// Preflight reported size bound, not a hard limit on provider transfer bytes.
        #[arg(long, default_value_t = 33554432, value_parser = clap::value_parser!(u64).range(1..=268435456))]
        max_bytes: u64,
        /// Submission deadline; killing the worker does not cancel provider downloads.
        #[arg(long, default_value_t = 10000, value_parser = clap::value_parser!(u64).range(1..=30000))]
        timeout_ms: u64,
    },
}

impl CloudCommand {
    fn request(self) -> (CloudRequest, u64) {
        let (scope, action) = match self {
            Self::Status { scope } => (scope, CloudAction::Status),
            Self::Download {
                root,
                path,
                expected_version,
                follow_links,
                max_bytes,
                timeout_ms,
            } => (
                ScopeArgs {
                    root,
                    path,
                    expected_version: Some(expected_version),
                    follow_links,
                    timeout_ms,
                },
                CloudAction::Download { max_bytes },
            ),
        };
        (
            CloudRequest {
                root: scope.root,
                path: scope.path,
                follow_links: scope.follow_links,
                expected_version: scope.expected_version,
                action,
            },
            scope.timeout_ms,
        )
    }
}

/// Dispatch a bounded cloud observation or explicit download submission.
pub(super) fn dispatch(action: CloudCommand) {
    let (request, timeout) = action.request();
    let result = std::env::current_exe()
        .map_err(FileError::from)
        .and_then(|exe| run(&exe, &request, timeout));
    output("files", result);
}

/// Execute one decoded cloud request in the supervised worker process.
pub(crate) fn worker() {
    output(
        "files/worker",
        read_request().and_then(|request| execute_worker(&request)),
    );
}

#[cfg(test)]
#[path = "files_cloud_tests.rs"]
mod tests;
