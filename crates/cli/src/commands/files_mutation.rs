use super::*;
use cueward_adapter_macos::files::mutation::{
    self, MutationAction, MutationReceipt, MutationRequest, MutationStatus,
};

#[derive(Args)]
pub(crate) struct WriteScope {
    #[arg(long)]
    root: PathBuf,
    /// Version from files info on the destination's existing parent directory.
    #[arg(long)]
    expected_parent_version: String,
    #[arg(long, default_value_t = 10000, value_parser=clap::value_parser!(u64).range(1..=30000))]
    timeout_ms: u64,
}
#[derive(Args)]
pub(crate) struct MkdirArgs {
    #[command(flatten)]
    scope: WriteScope,
    /// One new directory path relative to root; parents must already exist.
    #[arg(long)]
    path: PathBuf,
}
#[derive(Args)]
pub(crate) struct CopyArgs {
    #[command(flatten)]
    scope: WriteScope,
    /// Source regular file relative to root; symlinks/aliases are unsupported.
    #[arg(long)]
    path: PathBuf,
    /// New destination relative to the same root; no overwriting or auto-renaming.
    #[arg(long)]
    destination: PathBuf,
    #[arg(long)]
    expected_version: String,
    #[arg(long, default_value_t = 67108864, value_parser=clap::value_parser!(u64).range(1..=268435456))]
    max_bytes: u64,
}
#[derive(Args)]
pub(crate) struct ReceiptArgs {
    #[arg(long)]
    operation_id: String,
}
#[derive(serde::Serialize)]
#[serde(tag = "operation", content = "result", rename_all = "snake_case")]
enum Response {
    Mkdir(Box<MutationReceipt>),
    Copy(Box<MutationReceipt>),
    Duplicate(Box<MutationReceipt>),
    Receipt(Box<MutationReceipt>),
}

pub(super) fn mkdir(args: MkdirArgs) {
    let request = MutationRequest {
        root: args.scope.root,
        destination: args.path,
        expected_parent_version: args.scope.expected_parent_version,
        action: MutationAction::Mkdir,
    };
    dispatch(request, args.scope.timeout_ms);
}
pub(super) fn copy(args: CopyArgs) {
    let request = MutationRequest {
        root: args.scope.root,
        destination: args.destination,
        expected_parent_version: args.scope.expected_parent_version,
        action: MutationAction::Copy {
            path: args.path,
            expected_version: args.expected_version,
            max_bytes: args.max_bytes,
        },
    };
    dispatch(request, args.scope.timeout_ms);
}
/// Dispatch supported writes with receipt-based completion semantics.
pub(super) fn dispatch(request: MutationRequest, timeout_ms: u64) {
    let result = std::env::current_exe()
        .map_err(FileError::from)
        .and_then(|exe| mutation::run(&exe, &request, timeout_ms));
    let success = result
        .as_ref()
        .is_ok_and(|r| r.status == MutationStatus::Completed && r.completion_verified);
    let result = result.map(|receipt| match request.action {
        MutationAction::Mkdir => Response::Mkdir(Box::new(receipt)),
        MutationAction::Copy { .. } => Response::Copy(Box::new(receipt)),
        MutationAction::Duplicate { .. } => Response::Duplicate(Box::new(receipt)),
    });
    output("files", result);
    if !success {
        std::process::exit(1);
    }
}
pub(super) fn receipt(args: ReceiptArgs) {
    output(
        "files",
        mutation::read_receipt(&args.operation_id).map(|r| Response::Receipt(Box::new(r))),
    );
}
pub(crate) fn worker() {
    output(
        "files/worker",
        mutation::read_supervised_request().and_then(|request| mutation::execute_worker(&request)),
    );
}

#[cfg(test)]
#[path = "files_mutation_tests.rs"]
mod tests;
