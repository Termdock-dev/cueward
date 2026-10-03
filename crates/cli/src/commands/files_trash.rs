use super::*;
use cueward_adapter_macos::files::trash;

#[derive(Subcommand)]
pub(crate) enum TrashCommand {
    /// Inspect one selected entry before deciding a trash operation; never move or delete.
    Plan(TrashPlanArgs),
}
#[derive(Args)]
pub(crate) struct TrashPlanArgs {
    #[arg(long)]
    root: PathBuf,
    /// Non-root relative entry; a leaf symlink is inspected without following it.
    #[arg(long)]
    path: PathBuf,
    /// Version from files info on this exact selected entry.
    #[arg(long)]
    expected_version: String,
    #[arg(long,default_value_t=10000,value_parser=clap::value_parser!(u64).range(1..=30000))]
    timeout_ms: u64,
}
#[derive(serde::Serialize)]
struct Response {
    operation: &'static str,
    result: trash::TrashPlan,
}
/// Return observations, never a successful removal or confirmation token.
pub(super) fn dispatch(command: TrashCommand) {
    let TrashCommand::Plan(args) = command;
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
