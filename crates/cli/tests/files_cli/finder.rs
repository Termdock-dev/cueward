use super::*;

fn finder(root: &Path, action: &str, args: &[&str]) -> Value {
    let output = command()
        .args(["files", "finder", action, "--root"])
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "invalid request must stop before desktop dispatch"
    );
    let result = envelope(&output.stdout, "files");
    assert!(result.get("Ok").is_none());
    result
}

#[test]
fn finder_cli_rejects_invalid_roots_and_scope_before_native_queries_or_reveal() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("a"), "unchanged").unwrap();
    assert_eq!(
        finder(Path::new("relative"), "context", &[])["Err"]["code"],
        "invalid_options"
    );
    assert_eq!(
        finder(&directory.path().join("missing"), "context", &[])["Err"]["code"],
        "not_found"
    );
    for (args, code) in [
        (vec!["--path", "missing"], "not_found"),
        (vec!["--path", "../outside"], "outside_root"),
        (
            vec!["--path", "a", "--expected-version", "stale"],
            "changed",
        ),
    ] {
        assert_eq!(
            finder(directory.path(), "reveal", &args)["Err"]["code"],
            code
        );
    }
    assert_eq!(fs::read(directory.path().join("a")).unwrap(), b"unchanged");
}

#[test]
fn finder_cli_reveal_rejects_leaf_symlinks_and_outside_targets() {
    use std::os::unix::fs::symlink;
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("a"), "unchanged").unwrap();
    symlink("a", directory.path().join("link")).unwrap();
    symlink("/etc/passwd", directory.path().join("outside")).unwrap();
    assert_eq!(
        finder(directory.path(), "reveal", &["--path", "link"])["Err"]["code"],
        "unsupported_type"
    );
    assert_eq!(
        finder(
            directory.path(),
            "reveal",
            &["--path", "outside", "--follow-links"]
        )["Err"]["code"],
        "outside_root"
    );
    assert_eq!(fs::read(directory.path().join("a")).unwrap(), b"unchanged");
}

#[test]
fn finder_worker_rejects_malformed_oversized_and_invalid_budgets() {
    let directory = tempfile::tempdir().unwrap();
    for bytes in [
        b"{".to_vec(),
        vec![b' '; 16385],
        serde_json::to_vec(
            &json!({"root":directory.path(),"action":{"operation":"context","max_items":501}}),
        )
        .unwrap(),
    ] {
        let mut child = command()
            .arg("files-finder-worker")
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
