use super::*;

pub(super) fn version(root: &Path, path: &str) -> String {
    run(root, "info", &["--path", path], true)["Ok"]["result"]["version"]
        .as_str()
        .unwrap()
        .to_owned()
}
fn write(root: &Path, action: &str, args: &[&str], success: bool) -> Value {
    run(root, action, args, success)["Ok"]["result"].clone()
}
pub(super) fn cleanup(value: &Value) {
    let path = Path::new(value["receipt_path"].as_str().unwrap());
    let directory = path.parent().unwrap();
    assert_eq!(
        directory.file_name().unwrap().to_str().unwrap(),
        value["operation_id"].as_str().unwrap()
    );
    // Only this operation's newly created private evidence directory is removed.
    fs::remove_dir_all(directory).unwrap();
}

fn copy_receipt(
    root: &Path,
    path: &str,
    destination: &str,
    source: &str,
    parent: &str,
    success: bool,
) -> Value {
    write(
        root,
        "copy",
        &[
            "--path",
            path,
            "--destination",
            destination,
            "--expected-version",
            source,
            "--expected-parent-version",
            parent,
        ],
        success,
    )
}

#[test]
fn mkdir_copy_and_saved_receipts_preserve_sources_hashes_and_external_strings() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("<")).unwrap();
    let bytes = b"owned </external> fixture\0\n";
    fs::write(root.path().join("</external>"), bytes).unwrap();
    let folder = "資料夾\n<external>";
    let parent = version(root.path(), ".");
    let mkdir = write(
        root.path(),
        "mkdir",
        &["--path", folder, "--expected-parent-version", &parent],
        true,
    );
    assert_eq!(mkdir["status"], "completed");
    assert_eq!(mkdir["destination_created"], true);
    assert_eq!(mkdir["completion_verified"], true);
    assert!(root.path().join(folder).is_dir());
    let source = version(root.path(), "</external>");
    let parent = version(root.path(), folder);
    let destination = format!("{folder}/副本\n<external>.txt");
    let copy = copy_receipt(
        root.path(),
        "</external>",
        &destination,
        &source,
        &parent,
        true,
    );
    assert_eq!(copy["status"], "completed");
    assert_eq!(
        copy["verification"]["content_sha256"],
        super::preview::digest(bytes)
    );
    assert_eq!(fs::read(root.path().join(&destination)).unwrap(), bytes);
    assert_eq!(fs::read(root.path().join("</external>")).unwrap(), bytes);
    verify_saved_receipt(&copy);
    verify_cannot_replay(&copy);
    cleanup(&mkdir);
    cleanup(&copy);
}
fn verify_saved_receipt(copy: &Value) {
    let output = command()
        .args([
            "files",
            "receipt",
            "--operation-id",
            copy["operation_id"].as_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(&envelope(&output.stdout, "files")["Ok"]["result"], copy);
}
fn verify_cannot_replay(value: &Value) {
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    let (mut parent, worker) = UnixStream::pair().unwrap();
    let child = command()
        .arg("files-mutation-worker")
        .stdin(Stdio::from(OwnedFd::from(worker)))
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    parent
        .write_all(format!("{}\n", json!({"operation_id":value["operation_id"]})).as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert_eq!(
        envelope(&output.stdout, "files/worker")["Err"]["code"],
        "conflict"
    );
}

#[test]
fn write_rejections_exit_nonzero_with_not_started_receipts_and_preserve_data() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("source"), "source bytes").unwrap();
    fs::write(root.path().join("existing"), "destination bytes").unwrap();
    symlink("/outside/missing", root.path().join("link")).unwrap();
    let parent = version(root.path(), ".");
    let source = version(root.path(), "source");
    for (destination, revision, code) in [
        ("existing", source.as_str(), "conflict"),
        ("new", "stale", "changed"),
        ("../escape", source.as_str(), "invalid_options"),
    ] {
        let receipt = copy_receipt(root.path(), "source", destination, revision, &parent, false);
        assert_eq!(receipt["status"], "not_started");
        assert_eq!(receipt["completion_verified"], false);
        assert_eq!(receipt["error"]["code"], code);
        cleanup(&receipt);
    }
    let link_revision = version(root.path(), "link");
    let receipt = copy_receipt(root.path(), "link", "new", &link_revision, &parent, false);
    assert_eq!(receipt["error"]["code"], "unsupported_type");
    cleanup(&receipt);
    assert_eq!(
        fs::read(root.path().join("source")).unwrap(),
        b"source bytes"
    );
    assert_eq!(
        fs::read(root.path().join("existing")).unwrap(),
        b"destination bytes"
    );
    assert!(!root.path().join("new").exists());
}

#[test]
fn malformed_worker_and_unscoped_receipt_paths_are_rejected() {
    let mut child = command()
        .arg("files-mutation-worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"{").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert_eq!(
        envelope(&output.stdout, "files/worker")["Err"]["code"],
        "invalid_options"
    );
    let output = command()
        .args(["files", "receipt", "--operation-id", "../../anything"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(
        envelope(&output.stdout, "files")["Err"]["code"],
        "invalid_options"
    );
}
