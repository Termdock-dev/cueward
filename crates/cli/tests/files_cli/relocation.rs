use super::mutation::{cleanup, version};
use super::*;
use std::os::unix::fs::MetadataExt;

fn invoke(
    root: &Path,
    action: &str,
    path: &str,
    destination: &str,
    parent: &str,
    dry_run: bool,
    success: bool,
) -> Value {
    let flag = if action == "rename" {
        "--name"
    } else {
        "--destination"
    };
    let source_version = version(root, path);
    let parent_version = version(root, parent);
    let mut args = vec![
        "--path",
        path,
        flag,
        destination,
        "--expected-version",
        &source_version,
        "--expected-parent-version",
        &parent_version,
    ];
    if dry_run {
        args.push("--dry-run");
    }
    run(root, action, &args, success)["Ok"]["result"].clone()
}
#[test]
fn relocation_cli_dry_run_move_rename_noop_and_receipt_keep_exact_bytes() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("to")).unwrap();
    fs::write(root.path().join("owned"), b"OWNED CLI\0").unwrap();
    let inode = root.path().join("owned").metadata().unwrap().ino();
    let plan = invoke(
        root.path(),
        "move",
        "owned",
        "to/臺灣\n<external>",
        "to",
        true,
        true,
    );
    assert_eq!(plan["same_filesystem"], true);
    assert!(plan["destination_before"].is_null());
    assert!(root.path().join("owned").exists());
    let moved = invoke(
        root.path(),
        "move",
        "owned",
        "to/臺灣\n<external>",
        "to",
        false,
        true,
    );
    assert_eq!(moved["status"], "completed");
    assert_eq!(moved["source_path_absent"], true);
    assert_eq!(
        root.path()
            .join("to/臺灣\n<external>")
            .metadata()
            .unwrap()
            .ino(),
        inode
    );
    finish_rename_and_receipt(root.path(), &moved);
}

fn finish_rename_and_receipt(root: &Path, moved: &Value) {
    let output = command()
        .args([
            "files",
            "relocation",
            "receipt",
            "--operation-id",
            moved["operation_id"].as_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(&envelope(&output.stdout, "files")["Ok"]["result"], moved);
    let renamed = invoke(
        root,
        "rename",
        "to/臺灣\n<external>",
        "second",
        "to",
        false,
        true,
    );
    let noop = invoke(root, "rename", "to/second", "second", "to", false, true);
    assert_eq!(noop["mutation_attempted"], false);
    assert_eq!(fs::read(root.join("to/second")).unwrap(), b"OWNED CLI\0");
    for receipt in [moved, &renamed, &noop] {
        cleanup(receipt);
    }
}

#[test]
fn relocation_conflict_exit_is_nonzero_even_with_an_ok_receipt() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("source"), b"OWNED source").unwrap();
    fs::write(root.path().join("destination"), b"OWNED destination").unwrap();
    let plan = invoke(
        root.path(),
        "move",
        "source",
        "destination",
        ".",
        true,
        true,
    );
    assert_eq!(plan["destination_before"]["kind"], "file");
    let receipt = invoke(
        root.path(),
        "move",
        "source",
        "destination",
        ".",
        false,
        false,
    );
    assert_eq!(receipt["error"]["code"], "conflict");
    assert_eq!(receipt["status"], "not_started");
    assert_eq!(
        fs::read(root.path().join("source")).unwrap(),
        b"OWNED source"
    );
    assert_eq!(
        fs::read(root.path().join("destination")).unwrap(),
        b"OWNED destination"
    );
    cleanup(&receipt);
}
#[test]
fn relocation_invalid_name_has_structured_error_without_receipt_or_writer() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("source"), b"OWNED source").unwrap();
    let result = run(
        root.path(),
        "rename",
        &[
            "--path",
            "source",
            "--name",
            "sub/name",
            "--expected-version",
            "s",
            "--expected-parent-version",
            "p",
        ],
        false,
    );
    assert_eq!(result["Err"]["code"], "invalid_options");
    assert!(root.path().join("source").exists());
}
