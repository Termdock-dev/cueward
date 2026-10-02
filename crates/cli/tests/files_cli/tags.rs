//! Native tag editing, external-envelope semantics and parent cancellation on owned fixtures.
use super::cancellation::{Processes, wait_for};
use super::*;
use cueward_adapter_macos::files::tags::{self, TagsRequest};
use cueward_core::files::tags::TagEdit;
use std::os::unix::fs::PermissionsExt;

fn invoke(root: &Path, action: &str, args: &[&str], success: bool) -> Value {
    let output = command()
        .args(["files", "tags", action, "--root"])
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert_eq!(
        output.status.success(),
        success,
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let value = envelope(&output.stdout, "files");
    if action != "read" && value["Ok"]["result"]["operation_id"].is_string() {
        let id = value["Ok"]["result"]["operation_id"].as_str().unwrap();
        assert!(String::from_utf8_lossy(&output.stderr).contains(&format!("operation_id={id}")));
        assert!(String::from_utf8_lossy(&output.stderr).contains("files tags receipt"));
    }
    value["Ok"]["result"].clone()
}
fn edit(root: &Path, action: &str, snapshot: &Value, name: &str, success: bool) -> Value {
    invoke(
        root,
        action,
        &[
            "--path",
            "owned\n<external>",
            "--expected-version",
            snapshot["file"]["version"].as_str().unwrap(),
            "--expected-tags-version",
            snapshot["tags_version"].as_str().unwrap(),
            "--tag",
            name,
        ],
        success,
    )
}
fn snapshot(root: &Path) -> Value {
    invoke(root, "read", &["--path", "owned\n<external>"], true)
}
#[test]
fn tags_cli_initial_add_rejected_edits_receipts_and_stale_guards_preserve_payload() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("owned\n<external>");
    let bytes = b"OWNED tag content\0";
    fs::write(&path, bytes).unwrap();
    let before = snapshot(root.path());
    assert_eq!(before["tags"], json!([]));
    let name = "工作 </external>";
    let added = edit(root.path(), "add", &before, name, true);
    assert_eq!(added["status"], "completed");
    assert_eq!(added["after"]["tags"][0]["name"], name);
    assert_eq!(added["after"]["tags"][0]["color"], 0);
    verify_receipt(&added);
    let stale = edit(root.path(), "remove", &before, name, false);
    assert_eq!(stale["status"], "not_started");
    let no_op = edit(root.path(), "add", &snapshot(root.path()), name, true);
    assert_eq!(no_op["changed_by_operation"], false);
    let current = snapshot(root.path());
    let rejected_add = edit(root.path(), "add", &current, "Other", false);
    let removed = edit(root.path(), "remove", &current, name, false);
    for rejected in [&rejected_add, &removed] {
        assert_eq!(rejected["status"], "not_started");
        assert_eq!(rejected["error"]["code"], "unsupported_type");
        assert_eq!(rejected["mutation_attempted"], false);
        verify_receipt(rejected);
    }
    let missing = edit(root.path(), "remove", &current, "Missing", true);
    assert_eq!(missing["changed_by_operation"], false);
    assert_eq!(snapshot(root.path())["tags"], current["tags"]);
    assert_eq!(fs::read(path).unwrap(), bytes);
    for receipt in [&added, &stale, &no_op, &rejected_add, &removed, &missing] {
        super::mutation::cleanup(receipt);
    }
}
fn verify_receipt(receipt: &Value) {
    let id = receipt["operation_id"].as_str().unwrap();
    let output = command()
        .args(["files", "tags", "receipt", "--operation-id", id])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(&envelope(&output.stdout, "files")["Ok"]["result"], receipt);
    assert_eq!(
        fs::metadata(receipt["receipt_path"].as_str().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}
#[test]
fn tags_cli_rejects_empty_names_without_starting_a_writer() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("owned\n<external>"), b"owned").unwrap();
    let snapshot = snapshot(root.path());
    let output = command()
        .args(["files", "tags", "add", "--root"])
        .arg(root.path())
        .args([
            "--path",
            "owned\n<external>",
            "--expected-version",
            snapshot["file"]["version"].as_str().unwrap(),
            "--expected-tags-version",
            snapshot["tags_version"].as_str().unwrap(),
            "--tag",
            "",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(
        envelope(&output.stdout, "files")["Err"]["code"],
        "invalid_options"
    );
    assert!(output.stderr.is_empty());
}
#[test]
fn tag_cancellation_parent_helper() {
    let Some(config) = std::env::var_os("CUEWARD_TEST_TAG_CONFIG") else {
        return;
    };
    let value: Value = serde_json::from_slice(&fs::read(config).unwrap()).unwrap();
    let request = serde_json::from_value::<TagsRequest>(value["request"].clone()).unwrap();
    let _ = tags::run(
        Path::new(value["worker"].as_str().unwrap()),
        &request,
        30000,
    );
}
fn launch(directory: &Path, request: &TagsRequest) -> Processes {
    let wrapper = directory.join("worker");
    fs::write(&wrapper, format!("#!/bin/sh\nprintf '%s' \"$$\" > '{0}/group'\nprintf ready > '{0}/ready'\nwhile [ ! -e '{0}/release' ]; do /bin/sleep 0.01; done\n'{1}' files-tags-worker\nprintf '%s' \"$?\" > '{0}/finished'\n", directory.display(), env!("CARGO_BIN_EXE_cueward"))).unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700)).unwrap();
    let config = directory.join("config.json");
    fs::write(
        &config,
        serde_json::to_vec(&json!({"request":request,"worker":wrapper})).unwrap(),
    )
    .unwrap();
    let parent = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "tags::tag_cancellation_parent_helper",
            "--nocapture",
        ])
        .env("CUEWARD_TEST_TAG_CONFIG", config)
        .stdout(Stdio::null())
        .stderr(Stdio::from(
            fs::File::create(directory.join("progress")).unwrap(),
        ))
        .spawn()
        .unwrap();
    Processes {
        parent: Some(parent),
        worker: None,
    }
}
fn interrupt(signal: &str) {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("root");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("owned"), b"OWNED cancellation content").unwrap();
    let before = tags::read(&root, Path::new("owned"), None).unwrap();
    let request = TagsRequest {
        root: root.clone(),
        path: "owned".into(),
        expected_version: before.file.version.clone(),
        expected_tags_version: before.tags_version.clone(),
        edit: TagEdit::Add(vec!["Never".into()]),
    };
    let mut processes = launch(fixture.path(), &request);
    wait_for(|| fixture.path().join("ready").exists());
    processes.worker = Some(fs::read_to_string(fixture.path().join("group")).unwrap());
    let parent = processes.parent.as_mut().unwrap();
    assert!(
        Command::new("/bin/kill")
            .args([signal, &parent.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    parent.wait().unwrap();
    processes.parent = None;
    fs::write(fixture.path().join("release"), b"owned barrier").unwrap();
    wait_for(|| fs::metadata(fixture.path().join("finished")).is_ok_and(|m| m.len() > 0));
    check_cancelled(fixture.path(), &root, &before.tags_version);
}
fn check_cancelled(directory: &Path, root: &Path, before_version: &str) {
    let progress = fs::read_to_string(directory.join("progress")).unwrap();
    let id = progress
        .split("operation_id=")
        .nth(1)
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    let receipt = tags::read_receipt(id).unwrap();
    assert!(!receipt.completion_verified);
    assert!(!receipt.mutation_attempted);
    assert_eq!(
        tags::read(root, Path::new("owned"), None)
            .unwrap()
            .tags_version,
        before_version
    );
    assert_eq!(
        fs::read(root.join("owned")).unwrap(),
        b"OWNED cancellation content"
    );
    super::mutation::cleanup(&serde_json::to_value(receipt).unwrap());
}
#[test]
fn tag_parent_interruption_rejects_delayed_writer_startup() {
    for signal in ["-INT", "-TERM", "-KILL"] {
        interrupt(signal);
    }
}
