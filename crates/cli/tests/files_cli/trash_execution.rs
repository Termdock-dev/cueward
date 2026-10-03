use super::mutation::version;
use super::*;
use std::os::unix::fs::MetadataExt;

fn execute(root: &Path, path: &str, guard: &str, parent: &str) -> std::process::Output {
    command()
        .args(["files", "trash", "execute", "--root"])
        .arg(root)
        .args([
            "--path",
            path,
            "--expected-version",
            guard,
            "--expected-parent-version",
            parent,
            "--confirm",
        ])
        .output()
        .unwrap()
}
fn remove_owned_receipt(receipt: &Value) {
    let directory = Path::new(receipt["receipt_path"].as_str().unwrap())
        .parent()
        .unwrap();
    assert_eq!(
        directory.file_name().unwrap().to_str().unwrap(),
        receipt["operation_id"].as_str().unwrap()
    );
    fs::remove_dir_all(directory).unwrap();
}
#[test]
fn trash_execute_cli_stale_selection_is_nonzero_with_saved_not_started_evidence_and_no_backup() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("source"), b"OWNED bytes\0</external>").unwrap();
    let output = execute(root.path(), "source", "stale", &version(root.path(), "."));
    assert!(!output.status.success());
    let result = envelope(&output.stdout, "files");
    assert_eq!(result["Ok"]["operation"], "trash");
    let receipt = &result["Ok"]["result"];
    assert_eq!(receipt["status"], "not_started");
    assert_eq!(receipt["error"]["code"], "changed");
    assert_eq!(receipt["completion_verified"], false);
    assert!(receipt["backup_path"].is_null() && receipt["trash_path"].is_null());
    let id = receipt["operation_id"].as_str().unwrap();
    assert!(String::from_utf8_lossy(&output.stderr).contains(&format!("operation_id={id}")));
    let saved = command()
        .args(["files", "trash", "receipt", "--operation-id", id])
        .output()
        .unwrap();
    assert!(saved.status.success());
    assert_eq!(envelope(&saved.stdout, "files")["Ok"]["result"], *receipt);
    let wrong = command()
        .args(["files", "receipt", "--operation-id", id])
        .output()
        .unwrap();
    assert!(!wrong.status.success());
    assert_eq!(
        fs::read(root.path().join("source")).unwrap(),
        b"OWNED bytes\0</external>"
    );
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    remove_owned_receipt(receipt);
}
#[test]
fn trash_execute_cli_never_infers_confirmation_and_receipt_has_no_retry_or_restore() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("source"), b"OWNED").unwrap();
    let output = command()
        .args(["files", "trash", "execute", "--root"])
        .arg(root.path())
        .args([
            "--path",
            "source",
            "--expected-version",
            &version(root.path(), "source"),
            "--expected-parent-version",
            &version(root.path(), "."),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("operation_id="));
    assert_eq!(fs::read(root.path().join("source")).unwrap(), b"OWNED");
    let output = command()
        .args(["files", "trash", "receipt", "--operation-id", "../source"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(
        envelope(&output.stdout, "files")["Err"]["code"],
        "invalid_options"
    );
}
#[test]
fn trash_worker_rejects_unknown_and_unframed_requests_before_claim_or_namespace_writes() {
    for bytes in [
        b"{}".to_vec(),
        b"{\"operation_id\":\"x\",\"confirm\":true}\n".to_vec(),
        vec![b' '; 16385],
    ] {
        let mut child = command()
            .arg("files-trash-worker")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(&bytes).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(!output.status.success());
        assert_eq!(
            envelope(&output.stdout, "files/worker")["Err"]["code"],
            "invalid_options"
        );
    }
}
#[test]
#[ignore = "owned fixture only; requires native user Trash access and guarded same-volume rename support"]
fn trash_execute_cli_native_user_trash_owned_fixture_and_exact_manual_put_back() {
    let root = tempfile::tempdir().unwrap();
    let name = "cueward-owned-trash-臺灣\n<external>";
    let original = root.path().join(name);
    fs::write(&original, b"OWNED native acceptance\0</external>").unwrap();
    assert!(
        Command::new("/usr/bin/xattr")
            .args([
                "-w",
                "com.cueward.owned-native-trash",
                "OWNED original metadata"
            ])
            .arg(&original)
            .status()
            .unwrap()
            .success()
    );
    let before = fs::metadata(&original).unwrap();
    let output = execute(
        root.path(),
        name,
        &version(root.path(), name),
        &version(root.path(), "."),
    );
    let result = envelope(&output.stdout, "files");
    let receipt = &result["Ok"]["result"];
    if receipt["status"] != "completed" {
        eprintln!("Native owned-fixture refusal: {receipt}");
        // No own Trash entry exists on a not-started refusal. Keep uncertain evidence.
        if receipt["status"] == "not_started" {
            remove_owned_receipt(receipt);
        }
        assert!(
            output.status.success(),
            "native Trash acceptance did not complete"
        );
    }
    assert!(output.status.success());
    let trash = Path::new(receipt["trash_path"].as_str().unwrap());
    assert!(
        trash
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("cueward-owned-trash")
    );
    assert_eq!(fs::metadata(trash).unwrap().ino(), before.ino());
    assert_eq!(
        fs::read(trash).unwrap(),
        b"OWNED native acceptance\0</external>"
    );
    assert_eq!(
        fs::read(receipt["backup_path"].as_str().unwrap()).unwrap(),
        fs::read(trash).unwrap()
    );
    assert!(!original.exists());
    let attribute = Command::new("/usr/bin/xattr")
        .args(["-p", "com.cueward.owned-native-trash"])
        .arg(trash)
        .output()
        .unwrap();
    assert!(attribute.status.success());
    assert_eq!(
        String::from_utf8(attribute.stdout).unwrap().trim_end(),
        "OWNED original metadata"
    );
    assert_eq!(receipt["source_removed"], true);
    assert_eq!(receipt["moved_to_trash"], true);
    assert_eq!(receipt["staged_source_absent"], true);
    // Test cleanup touches only this exact verified owned item; no Trash enumeration/emptying.
    fs::rename(trash, &original).unwrap();
    assert_eq!(fs::metadata(&original).unwrap().ino(), before.ino());
    assert_eq!(
        fs::read(&original).unwrap(),
        b"OWNED native acceptance\0</external>"
    );
    assert!(!trash.exists());
    remove_owned_receipt(receipt);
}
