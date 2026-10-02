use super::*;
use cueward_adapter_macos::files::copy_tree;
use cueward_core::files::copy_tree::{CopyTreePlan, CopyTreeRequest};

#[derive(Subcommand)]
pub(crate) enum CopyTreeCommand {
    /// Inspect a bounded recursive directory copy; never execute or overwrite.
    Plan(CopyTreeArgs),
}
#[derive(Args)]
pub(crate) struct CopyTreeArgs {
    #[arg(long)]
    root: PathBuf,
    /// Source directory relative to root; packages are not traversed.
    #[arg(long)]
    path: PathBuf,
    /// Exact new relative directory path under an existing parent.
    #[arg(long)]
    destination: PathBuf,
    #[arg(long)]
    expected_version: String,
    /// Version from files info on the existing destination parent.
    #[arg(long)]
    expected_parent_version: String,
    /// Includes the top source directory and unsupported nodes.
    #[arg(long, default_value_t=256, value_parser=clap::value_parser!(u16).range(1..=256))]
    max_entries: u16,
    /// Source is depth zero; exceeding this bound returns no partial plan.
    #[arg(long, default_value_t=16, value_parser=clap::value_parser!(u8).range(1..=32))]
    max_depth: u8,
    /// Observed regular-file sizes, not bytes read or verified payloads.
    #[arg(long, default_value_t=67108864, value_parser=clap::value_parser!(u64).range(1..=268435456))]
    max_bytes: u64,
    #[arg(long, default_value_t=10000, value_parser=clap::value_parser!(u64).range(1..=30000))]
    timeout_ms: u64,
}
#[derive(serde::Serialize)]
struct Response {
    operation: &'static str,
    result: CopyTreePlan,
}
/// Plan only; outer Ok means observations returned, not permission or copy success.
pub(super) fn dispatch(command: CopyTreeCommand) {
    let CopyTreeCommand::Plan(args) = command;
    let request = CopyTreeRequest {
        root: args.root,
        path: args.path,
        destination: args.destination,
        expected_version: args.expected_version,
        expected_parent_version: args.expected_parent_version,
        max_entries: args.max_entries as usize,
        max_depth: args.max_depth as usize,
        max_bytes: args.max_bytes,
    };
    output(
        "files",
        std::env::current_exe()
            .map_err(FileError::from)
            .and_then(|exe| copy_tree::run_plan(&exe, &request, args.timeout_ms))
            .map(|result| Response {
                operation: "copy_tree_plan",
                result,
            }),
    );
}
/// Decode a bounded request before entering the read-only platform worker.
pub(crate) fn worker() {
    output(
        "files/worker",
        read_request().and_then(|r| copy_tree::plan_worker(&r)),
    );
}
#[cfg(test)]
#[path = "files_copy_tree_tests.rs"]
mod tests;
