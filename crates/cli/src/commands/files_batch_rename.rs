use super::*;
use cueward_adapter_macos::files::MAX_REQUEST_BYTES;
use cueward_adapter_macos::files::batch_execution::{self, BatchExecutionReceipt};
use cueward_adapter_macos::files::batch_rename::{
    self, BatchRenameEntry, BatchRenamePlan, BatchRenameRequest,
};
use cueward_core::files::relocation::RelocationStatus;
use cueward_core::files::relocation::batch::MAX_BATCH_RENAMES;

#[derive(Args)]
pub(crate) struct BatchRenameArgs {
    #[arg(long)]
    root: PathBuf,
    /// Repeat one JSON object with path, name, expected_version and expected_parent_version.
    #[arg(long = "entry", required = true, action = clap::ArgAction::Append)]
    entries: Vec<String>,
    #[arg(long, default_value_t = 10000, value_parser=clap::value_parser!(u64).range(1..=30000))]
    timeout_ms: u64,
}
#[derive(Subcommand)]
pub(crate) enum BatchRenameCommand {
    /// Observe explicit sibling proposals and conflicts; never rename or produce an execution token.
    Plan(BatchRenameArgs),
    /// Recheck independent entries and rename in order; stop on failure, never retry or roll back.
    Execute(BatchRenameArgs),
    /// Read saved aggregate evidence without resuming any pending entries.
    Receipt {
        #[arg(long)]
        operation_id: String,
    },
}

#[derive(serde::Serialize)]
#[serde(tag = "operation", content = "result", rename_all = "snake_case")]
enum Response {
    #[serde(rename = "rename_batch_plan")]
    Plan(Box<BatchRenamePlan>),
    #[serde(rename = "rename_batch")]
    Execute(Box<BatchExecutionReceipt>),
    #[serde(rename = "rename_batch_receipt")]
    Receipt(Box<BatchExecutionReceipt>),
}

/// Keep read-only plans separate from explicit execution and receipt lookup.
pub(super) fn dispatch(command: BatchRenameCommand) {
    let (args, execute) = match command {
        BatchRenameCommand::Plan(args) => (args, false),
        BatchRenameCommand::Execute(args) => (args, true),
        BatchRenameCommand::Receipt { operation_id } => {
            output(
                "files",
                batch_execution::read_receipt(&operation_id)
                    .map(|r| Response::Receipt(Box::new(r))),
            );
            return;
        }
    };
    let BatchRenameArgs {
        root,
        entries,
        timeout_ms,
    } = args;
    let request = parse_entries(&entries).map(|entries| BatchRenameRequest { root, entries });
    let input = request.and_then(|request| {
        std::env::current_exe()
            .map_err(FileError::from)
            .map(|exe| (exe, request))
    });
    if !execute {
        output(
            "files",
            input
                .and_then(|(exe, request)| batch_rename::run_plan(&exe, &request, timeout_ms))
                .map(|p| Response::Plan(Box::new(p))),
        );
        return;
    }
    let result = input.and_then(|(exe, request)| batch_execution::run(&exe, &request, timeout_ms));
    let success = result
        .as_ref()
        .is_ok_and(|r| r.status == RelocationStatus::Completed && r.completion_verified);
    output("files", result.map(|r| Response::Execute(Box::new(r))));
    if !success {
        std::process::exit(1);
    }
}

/// Arm one parent lifeline for the whole batch, including all child native operations.
pub(crate) fn execute_worker() {
    output(
        "files/worker",
        batch_execution::read_supervised_request()
            .and_then(|request| batch_execution::execute_worker(&request)),
    );
}

fn parse_entries(entries: &[String]) -> Result<Vec<BatchRenameEntry>, FileError> {
    if !(1..=MAX_BATCH_RENAMES).contains(&entries.len())
        || entries.iter().map(String::len).sum::<usize>() > MAX_REQUEST_BYTES
    {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "batch requires 1..64 entries within 16 KiB",
        ));
    }
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            serde_json::from_str(entry).map_err(|error| {
                FileError::new(
                    FileErrorCode::InvalidOptions,
                    format!("invalid JSON entry {index}: {error}"),
                )
            })
        })
        .collect()
}

/// Dedicated read-only protocol entry point; no mutation lifetime or receipt store.
pub(crate) fn worker() {
    output(
        "files/worker",
        read_request().and_then(|request| batch_rename::plan_worker(&request)),
    );
}

#[cfg(test)]
#[path = "files_batch_rename_tests.rs"]
mod tests;
