use super::*;
use cueward_adapter_macos::files::tags::{self, TagsReceipt, TagsRequest, TagsStatus};
use cueward_core::files::tags::{TagEdit, TagSnapshot};

#[derive(Subcommand)]
pub(crate) enum TagsCommand {
    /// Read names, stored colors and the exact tag revision without editing.
    Read(TagsReadArgs),
    /// Add exact names, preserving unselected tags/colors and saving the original attribute.
    Add(TagsWriteArgs),
    /// Remove only selected names, preserving other tags/colors; missing-name requests are no-ops.
    Remove(TagsWriteArgs),
    /// Read saved evidence only, without restoring tags or retrying.
    Receipt {
        #[arg(long)]
        operation_id: String,
    },
}
#[derive(Args)]
pub(crate) struct TagsReadArgs {
    #[arg(long)]
    root: PathBuf,
    #[arg(long, default_value = ".")]
    path: PathBuf,
    #[arg(long, default_value_t=10000, value_parser=clap::value_parser!(u64).range(1..=30000))]
    timeout_ms: u64,
}
#[derive(Args)]
pub(crate) struct TagsWriteArgs {
    #[arg(long)]
    root: PathBuf,
    /// Existing file/directory relative to root; no links/aliases/placeholders.
    #[arg(long)]
    path: PathBuf,
    #[arg(long)]
    expected_version: String,
    /// Exact revision returned by files tags read, not files metadata.
    #[arg(long)]
    expected_tags_version: String,
    /// Repeat for multiple exact names; color editing is not supported.
    #[arg(long, required=true, action=clap::ArgAction::Append)]
    tag: Vec<String>,
    #[arg(long, default_value_t=10000, value_parser=clap::value_parser!(u64).range(1..=30000))]
    timeout_ms: u64,
}
#[derive(serde::Serialize)]
#[serde(tag = "operation", content = "result", rename_all = "snake_case")]
enum Response {
    #[serde(rename = "tags_read")]
    Read(Box<TagSnapshot>),
    #[serde(rename = "tags_edit")]
    Edit(Box<TagsReceipt>),
    #[serde(rename = "tags_receipt")]
    Receipt(Box<TagsReceipt>),
}

/// Dispatch tag reads, explicit name edits or saved-evidence queries.
pub(super) fn dispatch(command: TagsCommand) {
    match command {
        TagsCommand::Read(args) => output(
            "files",
            std::env::current_exe()
                .map_err(FileError::from)
                .and_then(|exe| tags::run_read(&exe, &args.root, &args.path, args.timeout_ms))
                .map(|r| Response::Read(Box::new(r))),
        ),
        TagsCommand::Receipt { operation_id } => output(
            "files",
            tags::read_receipt(&operation_id).map(|r| Response::Receipt(Box::new(r))),
        ),
        TagsCommand::Add(args) => edit(args, true),
        TagsCommand::Remove(args) => edit(args, false),
    }
}
fn edit(args: TagsWriteArgs, add: bool) {
    let request = TagsRequest {
        root: args.root,
        path: args.path,
        expected_version: args.expected_version,
        expected_tags_version: args.expected_tags_version,
        edit: if add {
            TagEdit::Add(args.tag)
        } else {
            TagEdit::Remove(args.tag)
        },
    };
    let result = std::env::current_exe()
        .map_err(FileError::from)
        .and_then(|exe| tags::run(&exe, &request, args.timeout_ms));
    let success = result
        .as_ref()
        .is_ok_and(|r| r.status == TagsStatus::Completed && r.completion_verified);
    output("files", result.map(|r| Response::Edit(Box::new(r))));
    if !success {
        std::process::exit(1);
    }
}
/// Run the read-only tag query under the parent's normal deadline.
pub(crate) fn read_worker() {
    output(
        "files/worker",
        read_request::<(PathBuf, PathBuf)>()
            .and_then(|(root, path)| tags::read(&root, &path, None)),
    );
}
/// Start parent-lifetime monitoring before executing a one-use tag edit.
pub(crate) fn worker() {
    output(
        "files/worker",
        tags::read_supervised_request().and_then(|r| tags::execute_worker(&r)),
    );
}
#[cfg(test)]
#[path = "files_tags_tests.rs"]
mod tests;
