use super::*;
use cueward_adapter_macos::files::MAX_REQUEST_BYTES;
use cueward_adapter_macos::files::batch_rename::{
    self, BatchRenameEntry, BatchRenamePlan, BatchRenameRequest,
};
use cueward_core::files::relocation::batch::MAX_BATCH_RENAMES;

#[derive(Subcommand)]
pub(crate) enum BatchRenameCommand {
    /// Observe explicit sibling proposals and conflicts; never rename or produce an execution token.
    Plan {
        #[arg(long)]
        root: PathBuf,
        /// Repeat one JSON object with path, name, expected_version and expected_parent_version.
        #[arg(long = "entry", required = true, action = clap::ArgAction::Append)]
        entries: Vec<String>,
        #[arg(long, default_value_t = 10000, value_parser=clap::value_parser!(u64).range(1..=30000))]
        timeout_ms: u64,
    },
}

#[derive(serde::Serialize)]
#[serde(tag = "operation", content = "result", rename_all = "snake_case")]
enum Response {
    RenameBatchPlan(BatchRenamePlan),
}

/// Parse explicit entries, preserve order and emit one read-only batch result.
pub(super) fn dispatch(command: BatchRenameCommand) {
    let BatchRenameCommand::Plan {
        root,
        entries,
        timeout_ms,
    } = command;
    let result = parse_entries(&entries).and_then(|entries| {
        let executable = std::env::current_exe().map_err(FileError::from)?;
        batch_rename::run_plan(
            &executable,
            &BatchRenameRequest { root, entries },
            timeout_ms,
        )
    });
    output("files", result.map(Response::RenameBatchPlan));
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
