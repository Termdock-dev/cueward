use super::batch_rename::{entry, fixture};
use super::*;
use cueward_adapter_macos::files::batch_execution;
use cueward_adapter_macos::files::batch_rename::BatchRenameRequest;
use std::os::unix::fs::PermissionsExt;

fn execute(root: &Path, entries: &[String], success: bool) -> Value {
    let mut command = command();
    command
        .args(["files", "rename-batch", "execute", "--root"])
        .arg(root);
    for entry in entries {
        command.args(["--entry", entry]);
    }
    let output = command.output().unwrap();
    assert_eq!(
        output.status.success(),
        success,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value = envelope(&output.stdout, "files");
    if let Some(id) = value["Ok"]["result"]["operation_id"].as_str() {
        assert!(String::from_utf8_lossy(&output.stderr).contains(id));
        assert!(String::from_utf8_lossy(&output.stderr).contains("rename-batch receipt"));
    }
    value
}
pub(super) fn cleanup(receipt: &Value) {
    for child in receipt["items"].as_array().unwrap() {
        if child["receipt_path"].is_string() {
            super::mutation::cleanup(child);
        }
    }
    super::mutation::cleanup(receipt);
}
#[test]
fn batch_execute_cli_verifies_siblings_exact_output_and_aggregate_child_receipts() {
    let root = fixture();
    let entries = [
        entry(root.path(), "a", "臺灣\n<external>"),
        entry(root.path(), "b", "other"),
    ];
    let value = execute(root.path(), &entries, true);
    assert_eq!(value["Ok"]["operation"], "rename_batch");
    let receipt = &value["Ok"]["result"];
    assert_eq!(receipt["status"], "completed");
    assert_eq!(receipt["completion_verified"], true);
    for (index, item) in receipt["items"].as_array().unwrap().iter().enumerate() {
        assert_eq!(item["index"], index);
        assert_eq!(item["status"], "completed");
        let output = command()
            .args([
                "files",
                "relocation",
                "receipt",
                "--operation-id",
                item["operation_id"].as_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        let child = envelope(&output.stdout, "files");
        assert_eq!(child["Ok"]["result"]["completion_verified"], true);
        assert_eq!(child["Ok"]["result"]["operation_id"], item["operation_id"]);
    }
    let output = command()
        .args([
            "files",
            "rename-batch",
            "receipt",
            "--operation-id",
            receipt["operation_id"].as_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success() && output.stderr.is_empty());
    assert_eq!(&envelope(&output.stdout, "files")["Ok"]["result"], receipt);
    assert_eq!(
        fs::read(root.path().join("臺灣\n<external>")).unwrap(),
        b"OWNED a\0"
    );
    assert_eq!(fs::read(root.path().join("other")).unwrap(), b"OWNED b\0");
    assert!(!root.path().join("a").exists() && !root.path().join("b").exists());
    cleanup(receipt);
}
#[test]
fn batch_execute_cli_conflicts_stale_and_invalid_entries_do_not_rename() {
    let root = fixture();
    let mut stale: Value = serde_json::from_str(&entry(root.path(), "b", "new")).unwrap();
    stale["expected_version"] = "stale".into();
    for entries in [
        vec![entry(root.path(), "a", "b"), entry(root.path(), "b", "a")],
        vec![entry(root.path(), "a", "new"), stale.to_string()],
    ] {
        let value = execute(root.path(), &entries, false);
        let receipt = &value["Ok"]["result"];
        assert_eq!(receipt["status"], "not_started");
        assert!(
            receipt["items"]
                .as_array()
                .unwrap()
                .iter()
                .all(|i| i["operation_id"].is_null())
        );
        assert_eq!(fs::read(root.path().join("a")).unwrap(), b"OWNED a\0");
        assert_eq!(fs::read(root.path().join("b")).unwrap(), b"OWNED b\0");
        cleanup(receipt);
    }
    let value = execute(root.path(), &["{}".into()], false);
    assert_eq!(value["Err"]["code"], "invalid_options");
}
#[test]
fn batch_execute_timeout_returns_saved_uncertain_evidence_without_replay() {
    let root = fixture();
    let wrapper = root.path().join("slow");
    fs::write(&wrapper, "#!/bin/sh\n/bin/sleep 60\n").unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700)).unwrap();
    let request = BatchRenameRequest {
        root: root.path().into(),
        entries: vec![serde_json::from_str(&entry(root.path(), "a", "new")).unwrap()],
    };
    let receipt = batch_execution::run(&wrapper, &request, 20).unwrap();
    assert_eq!(
        receipt.status,
        cueward_core::files::relocation::RelocationStatus::Uncertain
    );
    assert!(!receipt.completion_verified && receipt.items[0].operation_id.is_none());
    assert_eq!(
        receipt.error.as_ref().unwrap().code,
        cueward_core::files::FileErrorCode::Timeout
    );
    assert_eq!(fs::read(root.path().join("a")).unwrap(), b"OWNED a\0");
    assert!(!root.path().join("new").exists());
    cleanup(&serde_json::to_value(&receipt).unwrap());
}
