use super::*;
use cueward_adapter_macos::files::copy_tree;
use cueward_core::files::copy_tree::{CopyTreePlan, CopyTreeRequest};

#[derive(Subcommand)]
pub(crate) enum CopyTreeCommand {
    /// Inspect a bounded recursive directory copy; never execute or overwrite.
    Plan(CopyTreeArgs),
    /// Copy a freshly observed tree privately, verify and publish once; never overwrite.
    Execute(TreeExecuteArgs),
    /// Read saved tree evidence only, without resume or cleanup.
    Receipt {
        #[arg(long)]
        operation_id: String,
    },
}
#[derive(Args)]
pub(crate) struct CopyTreeArgs {
    #[command(flatten)]
    scope: TreeScopeArgs,
    /// Includes the top source directory and unsupported nodes.
    #[arg(long, default_value_t=256, value_parser=clap::value_parser!(u16).range(1..=256))]
    max_entries: u16,
}
#[derive(Args)]
pub(crate) struct TreeExecuteArgs {
    #[command(flatten)]
    scope: TreeScopeArgs,
    /// Includes the top source directory and unsupported nodes.
    #[arg(long, default_value_t=64, value_parser=clap::value_parser!(u16).range(1..=64))]
    max_entries: u16,
}
#[derive(Args)]
pub(crate) struct TreeScopeArgs {
    #[arg(long)]
    root: PathBuf,
    /// Source directory relative to root; packages are skipped unless explicitly included.
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
    /// Source is depth zero; exceeding this bound prevents publication.
    #[arg(long, default_value_t=16, value_parser=clap::value_parser!(u8).range(1..=32))]
    max_depth: u8,
    /// Summed observed file sizes; execution also bounds each file transfer.
    #[arg(long, default_value_t=67108864, value_parser=clap::value_parser!(u64).range(1..=268435456))]
    max_bytes: u64,
    /// Traverse known package directories too; links and unsupported metadata remain blocked.
    #[arg(long)]
    include_packages: bool,
    #[arg(long, default_value_t=10000, value_parser=clap::value_parser!(u64).range(1..=30000))]
    timeout_ms: u64,
}
#[derive(serde::Serialize)]
struct Response {
    operation: &'static str,
    result: CopyTreePlan,
}
/// Keep plan observations, execution success and typed evidence lookup distinct.
pub(super) fn dispatch(command: CopyTreeCommand) {
    match command {
        CopyTreeCommand::Plan(args) => {
            let timeout = args.scope.timeout_ms;
            let request = request(args.scope, args.max_entries);
            output(
                "files",
                std::env::current_exe()
                    .map_err(FileError::from)
                    .and_then(|exe| copy_tree::run_plan(&exe, &request, timeout))
                    .map(|result| Response {
                        operation: "copy_tree_plan",
                        result,
                    }),
            );
        }
        CopyTreeCommand::Execute(args) => execute(args),
        CopyTreeCommand::Receipt { operation_id } => output(
            "files",
            copy_tree::execution::read_receipt(&operation_id).map(|result| TreeResponse {
                operation: "copy_tree_receipt",
                result,
            }),
        ),
    }
}
fn request(args: TreeScopeArgs, max_entries: u16) -> CopyTreeRequest {
    CopyTreeRequest {
        root: args.root,
        path: args.path,
        destination: args.destination,
        expected_version: args.expected_version,
        expected_parent_version: args.expected_parent_version,
        max_entries: max_entries as usize,
        max_depth: args.max_depth as usize,
        max_bytes: args.max_bytes,
        include_packages: args.include_packages,
    }
}
#[derive(serde::Serialize)]
struct TreeResponse {
    operation: &'static str,
    result: copy_tree::execution::TreeReceipt,
}
fn execute(args: TreeExecuteArgs) {
    let timeout = args.scope.timeout_ms;
    let request = request(args.scope, args.max_entries);
    let result = std::env::current_exe()
        .map_err(FileError::from)
        .and_then(|exe| copy_tree::execution::run(&exe, &request, timeout));
    let success = result.as_ref().is_ok_and(|r| {
        r.status == cueward_core::files::mutation::MutationStatus::Completed
            && r.completion_verified
    });
    output(
        "files",
        result.map(|result| TreeResponse {
            operation: "copy_tree",
            result,
        }),
    );
    if !success {
        std::process::exit(1);
    }
}
/// Arm lifetime monitoring before dispatching the one-use tree operation.
pub(crate) fn execute_worker() {
    output(
        "files/worker",
        copy_tree::execution::read_supervised_request()
            .and_then(|r| copy_tree::execution::execute_worker(&r)),
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
