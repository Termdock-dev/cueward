use super::*;
use cueward_adapter_macos::files::spotlight::{SpotlightRequest, execute_worker, run};

#[derive(Args)]
pub(crate) struct SpotlightArgs {
    /// Explicit absolute directory; no home/computer-wide fallback.
    #[arg(long)]
    root: PathBuf,
    /// Case-insensitive indexed text substring; no raw predicates, controls, * or ?.
    #[arg(long)]
    text: String,
    /// Direct children have depth 1. Deeper candidates are excluded before file probes.
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=32))]
    max_depth: u8,
    /// Include paths with dot components, without inspecting Finder invisible flags.
    #[arg(long)]
    hidden: bool,
    /// Index-candidate budget before depth/hidden/type filtering; overflow is an error.
    #[arg(long, default_value_t = 10000, value_parser = clap::value_parser!(u16).range(1..=10000))]
    max_candidates: u16,
    /// Return the first sorted candidates; no pagination or snapshot token.
    #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u16).range(1..=500))]
    limit: u16,
    #[arg(long, default_value_t = 10000, value_parser = clap::value_parser!(u64).range(1..=30000))]
    timeout_ms: u64,
}

impl SpotlightArgs {
    fn request(self) -> (SpotlightRequest, u64) {
        (
            SpotlightRequest {
                root: self.root,
                text: self.text,
                max_depth: self.max_depth.into(),
                hidden: self.hidden,
                max_candidates: self.max_candidates.into(),
                limit: self.limit.into(),
            },
            self.timeout_ms,
        )
    }
}

/// Query the native content index inside a deadline-controlled child process.
pub(super) fn dispatch(args: SpotlightArgs) {
    let (request, timeout) = args.request();
    let result = std::env::current_exe()
        .map_err(FileError::from)
        .and_then(|exe| run(&exe, &request, timeout));
    output("files", result);
}

/// Decode a bounded request and execute one static native index query.
pub(crate) fn worker() {
    output(
        "files/worker",
        read_request().and_then(|request| execute_worker(&request)),
    );
}

#[cfg(test)]
#[path = "files_spotlight_tests.rs"]
mod tests;
