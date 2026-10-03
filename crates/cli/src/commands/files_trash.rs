use super::*;
use cueward_adapter_macos::files::trash;

#[derive(Subcommand)]
pub(crate) enum TrashCommand {
    /// Inspect one selected entry before deciding a trash operation; never move or delete.
    Plan(TrashPlanArgs),
    /// Back up one ordinary file, verify and move it once into existing system Trash.
    Execute(TrashExecuteArgs),
    /// Restore verified retained backup to the original path; never overwrite or remove Trash.
    Restore(RestoreArgs),
    /// Read a saved restore receipt only; no retry, rollback or cleanup.
    RestoreReceipt {
        #[arg(long)]
        operation_id: String,
    },
    /// Read saved trash evidence only; no retry or cleanup.
    Receipt {
        #[arg(long)]
        operation_id: String,
    },
}
#[derive(Args)]
pub(crate) struct TrashPlanArgs {
    #[arg(long)]
    root: PathBuf,
    /// Non-root relative entry; plan can inspect a leaf link, execute refuses links.
    #[arg(long)]
    path: PathBuf,
    /// Version from files info on this exact selected entry.
    #[arg(long)]
    expected_version: String,
    #[arg(long,default_value_t=10000,value_parser=clap::value_parser!(u64).range(1..=30000))]
    timeout_ms: u64,
}
#[derive(Args)]
pub(crate) struct TrashExecuteArgs {
    #[command(flatten)]
    selection: TrashPlanArgs,
    /// Fresh version of the source parent, from files info.
    #[arg(long)]
    expected_parent_version: String,
    /// Required explicit confirmation of this exact selected file; never infer from a plan.
    #[arg(long, required = true)]
    confirm: bool,
    #[arg(long, default_value_t=67108864, value_parser=clap::value_parser!(u64).range(1..=268435456))]
    max_bytes: u64,
}
#[derive(Args)]
pub(crate) struct RestoreArgs {
    /// Original completed trash receipt UUID, not a restore UUID.
    #[arg(long)]
    operation_id: String,
    /// Explicit original root; recorded canonical path and identity must match.
    #[arg(long)]
    root: PathBuf,
    #[arg(long)]
    expected_parent_version: String,
    #[arg(long,default_value_t=10000,value_parser=clap::value_parser!(u64).range(1..=30000))]
    timeout_ms: u64,
}
#[derive(serde::Serialize)]
struct Response {
    operation: &'static str,
    result: trash::TrashPlan,
}
/// Keep read-only proposals, confirmed execution and typed receipt lookup distinct.
pub(super) fn dispatch(command: TrashCommand) {
    match command {
        TrashCommand::Plan(args) => plan(args),
        TrashCommand::Restore(args) => restore(args),
        TrashCommand::RestoreReceipt { operation_id } => output(
            "files",
            trash::restore::read_receipt(&operation_id).map(|result| RestoreResponse {
                operation: "trash_restore_receipt",
                result,
            }),
        ),
        TrashCommand::Execute(args) => execute(args),
        TrashCommand::Receipt { operation_id } => output(
            "files",
            trash::execution::read_receipt(&operation_id).map(|result| ExecutionResponse {
                operation: "trash_receipt",
                result,
            }),
        ),
    }
}
fn plan(args: TrashPlanArgs) {
    let request = trash::TrashPlanRequest {
        root: args.root,
        path: args.path,
        expected_version: args.expected_version,
    };
    output(
        "files",
        std::env::current_exe()
            .map_err(FileError::from)
            .and_then(|exe| trash::run_plan(&exe, &request, args.timeout_ms))
            .map(|result| Response {
                operation: "trash_plan",
                result,
            }),
    );
}
/// Decode a bounded read-only request, then inspect metadata under the platform policy.
pub(crate) fn worker() {
    output(
        "files/worker",
        read_request().and_then(|request| trash::plan_worker(&request)),
    );
}
#[cfg(test)]
#[path = "files_trash_tests.rs"]
mod tests;

#[derive(serde::Serialize)]
struct ExecutionResponse {
    operation: &'static str,
    result: trash::execution::TrashReceipt,
}
fn execute(args: TrashExecuteArgs) {
    let timeout = args.selection.timeout_ms;
    let request = trash::execution::Request {
        root: args.selection.root,
        path: args.selection.path,
        expected_version: args.selection.expected_version,
        expected_parent_version: args.expected_parent_version,
        confirm: args.confirm,
        max_bytes: args.max_bytes,
    };
    let result = std::env::current_exe()
        .map_err(FileError::from)
        .and_then(|exe| trash::execution::run(&exe, &request, timeout));
    let success = result.as_ref().is_ok_and(|r| {
        r.status == cueward_core::files::mutation::MutationStatus::Completed
            && r.completion_verified
    });
    output(
        "files",
        result.map(|result| ExecutionResponse {
            operation: "trash",
            result,
        }),
    );
    if !success {
        std::process::exit(1);
    }
}
/// Arm parent-lifetime monitoring before claiming or reading a prepared trash operation.
pub(crate) fn execute_worker() {
    output(
        "files/worker",
        trash::execution::read_supervised_request()
            .and_then(|r| trash::execution::execute_worker(&r)),
    );
}

#[derive(serde::Serialize)]
struct RestoreResponse {
    operation: &'static str,
    result: trash::restore::RestoreReceipt,
}
fn restore(args: RestoreArgs) {
    let request = trash::restore::Request {
        trash_operation_id: args.operation_id,
        root: args.root,
        expected_parent_version: args.expected_parent_version,
    };
    let result = std::env::current_exe()
        .map_err(FileError::from)
        .and_then(|exe| trash::restore::run(&exe, &request, args.timeout_ms));
    let success = result.as_ref().is_ok_and(|r| {
        r.status == cueward_core::files::mutation::MutationStatus::Completed
            && r.completion_verified
    });
    output(
        "files",
        result.map(|result| RestoreResponse {
            operation: "trash_restore",
            result,
        }),
    );
    if !success {
        std::process::exit(1);
    }
}
/// Arm lifetime monitoring before any restore worker claim or filesystem work.
pub(crate) fn restore_worker() {
    output(
        "files/worker",
        trash::restore::read_supervised_request().and_then(|r| trash::restore::execute_worker(&r)),
    );
}
