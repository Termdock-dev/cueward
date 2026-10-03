use super::mutation::version;
use super::trash_execution::{execute, remove_owned_receipt};
use super::*;
use std::os::unix::fs::MetadataExt;
fn restore(root: &Path, id: &str, parent: &str) -> std::process::Output {
    command()
        .args(["files", "trash", "restore", "--operation-id", id, "--root"])
        .arg(root)
        .args(["--expected-parent-version", parent])
        .output()
        .unwrap()
}
#[test]
fn restore_cli_missing_original_saves_not_started_receipt_with_new_id_and_nonzero_exit() {
    let root = tempfile::tempdir().unwrap();
    let missing = "00000000-0000-0000-0000-000000000000";
    let output = restore(root.path(), missing, &version(root.path(), "."));
    assert!(!output.status.success());
    let result = envelope(&output.stdout, "files");
    assert_eq!(result["Ok"]["operation"], "trash_restore");
    let receipt = &result["Ok"]["result"];
    assert_eq!(receipt["status"], "not_started");
    assert!(!receipt["operation_id"].as_str().unwrap().eq(missing));
    assert_eq!(receipt["completion_verified"], false);
    assert!(receipt["staging_path"].is_null());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    let id = receipt["operation_id"].as_str().unwrap();
    assert!(String::from_utf8_lossy(&output.stderr).contains(&format!("operation_id={id}")));
    let saved = command()
        .args(["files", "trash", "restore-receipt", "--operation-id", id])
        .output()
        .unwrap();
    assert!(saved.status.success());
    assert_eq!(
        envelope(&saved.stdout, "files")["Ok"]["operation"],
        "trash_restore_receipt"
    );
    assert_eq!(envelope(&saved.stdout, "files")["Ok"]["result"], *receipt);
    let wrong = command()
        .args(["files", "trash", "receipt", "--operation-id", id])
        .output()
        .unwrap();
    assert!(!wrong.status.success());
    remove_owned_receipt(receipt);
}
#[test]
fn restore_worker_rejects_unknown_unframed_and_oversized_requests() {
    for bytes in [
        b"{}".to_vec(),
        b"{\"operation_id\":\"id\",\"root\":\"/owned\"}\n".to_vec(),
        vec![b' '; 16385],
    ] {
        let mut child = command()
            .arg("files-trash-restore-worker")
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
fn restore_cli_native_owned_backup_copy_keeps_exact_trash_entry_and_original_receipt() {
    let root = tempfile::tempdir().unwrap();
    let name = "cueward-owned-restore-臺灣\n<external>";
    let original = root.path().join(name);
    let bytes = b"OWNED native restore\0</external>";
    fs::write(&original, bytes).unwrap();
    assert!(
        Command::new("/usr/bin/xattr")
            .args(["-w", "com.cueward.owned-native-restore", "OWNED metadata"])
            .arg(&original)
            .status()
            .unwrap()
            .success()
    );
    let output = execute(
        root.path(),
        name,
        &version(root.path(), name),
        &version(root.path(), "."),
    );
    let result = envelope(&output.stdout, "files");
    let receipt = &result["Ok"]["result"];
    if !output.status.success() {
        let retained_root = root.keep();
        panic!(
            "native trash failed; retain root {} and evidence: {receipt}",
            retained_root.display()
        );
    }
    assert_eq!(receipt["status"], "completed");
    let trash = Path::new(receipt["trash_path"].as_str().unwrap());
    let backup = Path::new(receipt["backup_path"].as_str().unwrap());
    let trash_inode = fs::metadata(trash).unwrap().ino();
    let backup_inode = fs::metadata(backup).unwrap().ino();
    let original_json = fs::read(receipt["receipt_path"].as_str().unwrap()).unwrap();
    let output = restore(
        root.path(),
        receipt["operation_id"].as_str().unwrap(),
        &version(root.path(), "."),
    );
    let restored = envelope(&output.stdout, "files");
    let restore_receipt = &restored["Ok"]["result"];
    if !output.status.success() {
        let retained_root = root.keep();
        panic!(
            "native restore failed; retain root {} and both receipts: {restore_receipt}",
            retained_root.display()
        );
    }
    assert_eq!(restore_receipt["status"], "completed");
    assert_eq!(restore_receipt["completion_verified"], true);
    assert_eq!(restore_receipt["trash_item_touched"], false);
    assert_eq!(restore_receipt["backup_removed"], false);
    assert_eq!(fs::read(&original).unwrap(), bytes);
    assert_eq!(fs::read(trash).unwrap(), bytes);
    assert_eq!(fs::read(backup).unwrap(), bytes);
    assert_eq!(fs::metadata(trash).unwrap().ino(), trash_inode);
    assert_eq!(fs::metadata(backup).unwrap().ino(), backup_inode);
    assert_ne!(fs::metadata(&original).unwrap().ino(), trash_inode);
    assert_ne!(fs::metadata(&original).unwrap().ino(), backup_inode);
    assert_eq!(
        fs::read(receipt["receipt_path"].as_str().unwrap()).unwrap(),
        original_json
    );
    let attribute = Command::new("/usr/bin/xattr")
        .args(["-p", "com.cueward.owned-native-restore"])
        .arg(&original)
        .output()
        .unwrap();
    assert!(attribute.status.success());
    assert_eq!(
        String::from_utf8(attribute.stdout).unwrap().trim_end(),
        "OWNED metadata"
    );
    // Cleanup only this exact test-owned Trash item after verified restored copy, never enumerate Trash.
    assert!(
        trash
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("cueward-owned-restore")
    );
    fs::remove_file(trash).unwrap();
    remove_owned_receipt(restore_receipt);
    remove_owned_receipt(receipt);
}
