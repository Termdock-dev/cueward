use super::mutation::{cleanup, version};
use super::*;
use std::os::fd::OwnedFd;
use std::os::unix::fs::{MetadataExt, symlink};
use std::os::unix::net::UnixStream;

fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("source/sub/empty")).unwrap();
    fs::write(root.path().join("source/.hidden"), b"OWNED\0 bytes").unwrap();
    fs::write(root.path().join("source/sub/臺灣\n<external>"), b"nested").unwrap();
    root
}
fn execute(root: &Path, success: bool, extra: &[&str]) -> Value {
    let output = command()
        .args(["files", "copy-tree", "execute", "--root"])
        .arg(root)
        .args([
            "--path",
            "source",
            "--destination",
            "副本\n<external>",
            "--expected-version",
            &version(root, "source"),
            "--expected-parent-version",
            &version(root, "."),
        ])
        .args(extra)
        .output()
        .unwrap();
    assert_eq!(
        output.status.success(),
        success,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value = envelope(&output.stdout, "files");
    assert_eq!(value["Ok"]["operation"], "copy_tree");
    let receipt = value["Ok"]["result"].clone();
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(receipt["operation_id"].as_str().unwrap())
    );
    receipt
}
fn lookup(receipt: &Value) {
    let id = receipt["operation_id"].as_str().unwrap();
    let output = command()
        .args(["files", "copy-tree", "receipt", "--operation-id", id])
        .output()
        .unwrap();
    assert!(output.status.success());
    let value = envelope(&output.stdout, "files");
    assert_eq!(value["Ok"]["operation"], "copy_tree_receipt");
    assert_eq!(&value["Ok"]["result"], receipt);
    let output = command()
        .args(["files", "receipt", "--operation-id", id])
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "generic single-file lookup must reject the tree schema"
    );
}
fn replay(receipt: &Value) {
    let (mut parent, worker) = UnixStream::pair().unwrap();
    let child = command()
        .arg("files-copy-tree-worker")
        .stdin(Stdio::from(OwnedFd::from(worker)))
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    parent
        .write_all(format!("{}\n", json!({"operation_id":receipt["operation_id"]})).as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert_eq!(
        envelope(&output.stdout, "files/worker")["Err"]["code"],
        "conflict"
    );
}
#[test]
fn tree_execute_cli_real_worker_publishes_verified_tree_and_typed_saved_receipt() {
    let root = fixture();
    let before = version(root.path(), "source");
    let receipt = execute(root.path(), true, &[]);
    assert_eq!(receipt["status"], "completed");
    assert_eq!(receipt["stage"], "finished");
    assert_eq!(receipt["completion_verified"], true);
    assert_eq!(receipt["destination_created"], true);
    assert_eq!(receipt["staging_verified"], true);
    let nodes = receipt["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 5);
    assert!(nodes[0]["verification"].is_null());
    assert_eq!(
        nodes[1]["verification"]["content_sha256"],
        super::preview::digest(b"OWNED\0 bytes")
    );
    for path in [".hidden", "sub/臺灣\n<external>"] {
        let source = root.path().join("source").join(path);
        let destination = root.path().join("副本\n<external>").join(path);
        assert_eq!(fs::read(&source).unwrap(), fs::read(&destination).unwrap());
        assert_ne!(
            fs::metadata(source).unwrap().ino(),
            fs::metadata(destination).unwrap().ino()
        );
    }
    assert!(root.path().join("副本\n<external>/sub/empty").is_dir());
    assert_eq!(version(root.path(), "source"), before);
    assert!(!Path::new(receipt["staging_path"].as_str().unwrap()).exists());
    lookup(&receipt);
    replay(&receipt);
    cleanup(&receipt);
}
#[test]
fn tree_execute_cli_blockers_exit_nonzero_with_saved_not_started_evidence() {
    for case in ["existing", "link", "budget"] {
        let root = fixture();
        match case {
            "existing" => {
                fs::write(root.path().join("副本\n<external>"), b"OWNED existing").unwrap()
            }
            "link" => symlink("missing", root.path().join("source/broken")).unwrap(),
            _ => (),
        }
        let receipt = execute(
            root.path(),
            false,
            if case == "budget" {
                &["--max-entries", "1"]
            } else {
                &[]
            },
        );
        assert_eq!(receipt["status"], "not_started");
        assert_eq!(receipt["completion_verified"], false);
        assert_eq!(receipt["mutation_attempted"], false);
        assert!(receipt["staging_path"].is_null());
        assert!(!receipt["error"].is_null());
        if case == "existing" {
            assert_eq!(
                fs::read(root.path().join("副本\n<external>")).unwrap(),
                b"OWNED existing"
            );
        } else {
            assert!(!root.path().join("副本\n<external>").exists());
        }
        lookup(&receipt);
        cleanup(&receipt);
    }
}
#[test]
fn tree_execute_cli_rejects_plan_only_bounds_and_unsupported_options_before_receipts() {
    for flags in [
        ["--max-entries", "65"],
        ["--overwrite", "true"],
        ["--plan-file", "saved.json"],
    ] {
        let root = fixture();
        let output = command()
            .args(["files", "copy-tree", "execute", "--root"])
            .arg(root.path())
            .args([
                "--path",
                "source",
                "--destination",
                "copy",
                "--expected-version",
                "unused",
                "--expected-parent-version",
                "unused",
            ])
            .args(flags)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("operation_id="));
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}
#[test]
fn tree_execute_cli_worker_rejects_closed_parent_before_any_selected_write() {
    let root = fixture();
    let (mut parent, worker) = UnixStream::pair().unwrap();
    parent
        .write_all(b"{\"operation_id\":\"unused\"}\n")
        .unwrap();
    drop(parent);
    let child = command()
        .arg("files-copy-tree-worker")
        .stdin(Stdio::from(OwnedFd::from(worker)))
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    assert_eq!(
        envelope(&output.stdout, "files/worker")["Err"]["code"],
        "unavailable"
    );
}
