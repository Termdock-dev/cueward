use super::*;
use cueward_adapter_macos::files::relocation::{
    self, RelocationAction, RelocationPlan, RelocationReceipt, RelocationRequest, RelocationStatus,
};

#[derive(Args)]
pub(crate) struct RelocationScope {
    #[arg(long)]
    root: PathBuf,
    /// Existing relative file/directory/package; no links, aliases or downloads.
    #[arg(long)]
    path: PathBuf,
    #[arg(long)]
    expected_version: String,
    /// Version from files info on the existing destination parent.
    #[arg(long)]
    expected_parent_version: String,
    /// Observe only, including conflicts; no receipt or reusable execution token.
    #[arg(long)]
    dry_run: bool,
    #[arg(long, default_value_t=10000, value_parser=clap::value_parser!(u64).range(1..=30000))]
    timeout_ms: u64,
}
#[derive(Args)]
pub(crate) struct RenameArgs {
    #[command(flatten)]
    scope: RelocationScope,
    /// One exact new filename, not a path. Existing destinations are rejected.
    #[arg(long)]
    name: String,
}
#[derive(Args)]
pub(crate) struct MoveArgs {
    #[command(flatten)]
    scope: RelocationScope,
    /// New relative destination on the same volume; no overwrite or copy/delete fallback.
    #[arg(long)]
    destination: PathBuf,
}
#[derive(Subcommand)]
pub(crate) enum RelocationCommand {
    /// Read saved evidence without retrying or restoring names.
    Receipt {
        #[arg(long)]
        operation_id: String,
    },
}
#[derive(serde::Serialize)]
#[serde(tag = "operation", content = "result", rename_all = "snake_case")]
enum Response {
    Rename(Box<RelocationReceipt>),
    Move(Box<RelocationReceipt>),
    RenamePlan(Box<RelocationPlan>),
    MovePlan(Box<RelocationPlan>),
    RelocationReceipt(Box<RelocationReceipt>),
}
/// Keep the existing parent and validate one exact Unicode filename.
pub(super) fn rename(args: RenameArgs) {
    match cueward_core::files::relocation::rename_destination(
        &args.scope.path,
        std::path::Path::new(&args.name),
    ) {
        Ok(destination) => dispatch(args.scope, destination, RelocationAction::Rename),
        Err(error) => output::<Response>("files", Err(error)),
    }
}
/// Move an entry without copying bytes or deleting destination data.
pub(super) fn move_entry(args: MoveArgs) {
    dispatch(args.scope, args.destination, RelocationAction::Move);
}
fn dispatch(scope: RelocationScope, destination: PathBuf, action: RelocationAction) {
    let request = RelocationRequest {
        root: scope.root,
        path: scope.path,
        destination,
        expected_version: scope.expected_version,
        expected_parent_version: scope.expected_parent_version,
        action,
    };
    let exe = std::env::current_exe().map_err(FileError::from);
    if scope.dry_run {
        output(
            "files",
            exe.and_then(|exe| relocation::run_plan(&exe, &request, scope.timeout_ms))
                .map(|plan| match action {
                    RelocationAction::Rename => Response::RenamePlan(Box::new(plan)),
                    RelocationAction::Move => Response::MovePlan(Box::new(plan)),
                }),
        );
        return;
    }
    let result = exe.and_then(|exe| relocation::run(&exe, &request, scope.timeout_ms));
    let success = result
        .as_ref()
        .is_ok_and(|r| r.status == RelocationStatus::Completed && r.completion_verified);
    output(
        "files",
        result.map(|receipt| match action {
            RelocationAction::Rename => Response::Rename(Box::new(receipt)),
            RelocationAction::Move => Response::Move(Box::new(receipt)),
        }),
    );
    if !success {
        std::process::exit(1);
    }
}
/// Query the relocation namespace independently of copy/mkdir receipts.
pub(super) fn receipt(command: RelocationCommand) {
    let RelocationCommand::Receipt { operation_id } = command;
    output(
        "files",
        relocation::read_receipt(&operation_id).map(|r| Response::RelocationReceipt(Box::new(r))),
    );
}
/// Observe a proposal in a read-only worker.
pub(crate) fn plan_worker() {
    output(
        "files/worker",
        read_request().and_then(|request| relocation::plan_worker(&request)),
    );
}
/// Arm a parent lifeline and execute one private prepared request.
pub(crate) fn worker() {
    output(
        "files/worker",
        relocation::read_supervised_request()
            .and_then(|request| relocation::execute_worker(&request)),
    );
}
#[cfg(test)]
#[path = "files_relocation_tests.rs"]
mod tests;
