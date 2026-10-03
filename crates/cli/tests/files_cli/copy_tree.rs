use super::mutation::version;
use super::*;
use std::os::unix::fs::{MetadataExt, symlink};
fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("source/sub")).unwrap();
    fs::write(root.path().join("source/.hidden"), b"OWNED hidden").unwrap();
    fs::write(
        root.path().join("source/sub/臺灣\n<external>"),
        b"OWNED data\0",
    )
    .unwrap();
    root
}
fn plan(root: &Path, destination: &str, extra: &[&str], success: bool) -> Value {
    let parent = Path::new(destination)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let output = command()
        .args(["files", "copy-tree", "plan", "--root"])
        .arg(root)
        .args([
            "--path",
            "source",
            "--destination",
            destination,
            "--expected-version",
            &version(root, "source"),
            "--expected-parent-version",
            &version(root, parent.to_str().unwrap()),
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
    assert!(
        output.stderr.is_empty(),
        "read-only plan must not announce an operation receipt"
    );
    envelope(&output.stdout, "files")
}
#[test]
fn copy_tree_cli_real_worker_returns_sorted_observations_without_receipts_or_changes() {
    let root = fixture();
    let before = fs::metadata(root.path().join("source/.hidden")).unwrap();
    let value = plan(root.path(), "副本\n<external>", &[], true);
    assert_eq!(value["Ok"]["operation"], "copy_tree_plan");
    let result = &value["Ok"]["result"];
    assert_eq!(result["execution_supported"], false);
    assert_eq!(result["has_blockers"], false);
    assert_eq!(result["enumeration_complete"], true);
    assert_eq!(result["entries"].as_array().unwrap().len(), 4);
    assert_eq!(result["entries"][1]["relative_path"], ".hidden");
    assert_eq!(
        result["entries"][3]["destination"],
        "副本\n<external>/sub/臺灣\n<external>"
    );
    assert_eq!(
        result["entries"][0]["source"]["version"],
        result["request"]["expected_version"]
    );
    assert!(result.get("operation_id").is_none());
    assert!(result["entries"][3].get("content_sha256").is_none());
    let after = fs::metadata(root.path().join("source/.hidden")).unwrap();
    assert_eq!(
        (
            before.ino(),
            before.mode(),
            before.mtime(),
            before.mtime_nsec(),
            before.ctime(),
            before.ctime_nsec()
        ),
        (
            after.ino(),
            after.mode(),
            after.mtime(),
            after.mtime_nsec(),
            after.ctime(),
            after.ctime_nsec()
        )
    );
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    assert_eq!(
        fs::read(root.path().join("source/sub/臺灣\n<external>")).unwrap(),
        b"OWNED data\0"
    );
}
#[test]
fn copy_tree_cli_ok_can_carry_namespace_and_source_blockers() {
    let root = fixture();
    symlink("missing", root.path().join("copy")).unwrap();
    symlink("missing", root.path().join("source/broken")).unwrap();
    let value = plan(root.path(), "copy", &[], true);
    let result = &value["Ok"]["result"];
    assert_eq!(result["has_blockers"], true);
    assert_eq!(result["issues"], json!(["existing_destination"]));
    assert_eq!(result["destination_before"]["kind"], "symlink");
    let entries = result["entries"].as_array().unwrap();
    assert_eq!(
        entries
            .iter()
            .find(|e| e["relative_path"] == "broken")
            .unwrap()["error"],
        Value::Null
    );
    let value = plan(root.path(), "source/sub/copy", &[], true);
    assert_eq!(
        value["Ok"]["result"]["issues"],
        json!(["destination_within_source"])
    );
    assert!(!root.path().join("source/sub/copy").exists());
}
#[test]
fn copy_tree_cli_budget_and_stale_failures_discard_partial_observations() {
    let root = fixture();
    for flags in [
        ["--max-entries", "2"],
        ["--max-depth", "1"],
        ["--max-bytes", "1"],
    ] {
        let value = plan(root.path(), "copy", &flags, false);
        assert_eq!(value["Err"]["code"], "scan_limit");
        assert!(value.get("Ok").is_none());
    }
    let output = command()
        .args(["files", "copy-tree", "plan", "--root"])
        .arg(root.path())
        .args([
            "--path",
            "source",
            "--destination",
            "copy",
            "--expected-version",
            "stale",
            "--expected-parent-version",
            &version(root.path(), "."),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(envelope(&output.stdout, "files")["Err"]["code"], "changed");
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}
#[test]
fn copy_tree_cli_root_alias_stays_canonical_and_existing_commands_stay_compatible() {
    let root = fixture();
    let aliases = tempfile::tempdir().unwrap();
    let alias = aliases.path().join("link");
    symlink(root.path(), &alias).unwrap();
    let value = plan(&alias, "copy", &[], true);
    assert_eq!(
        value["Ok"]["result"]["request"]["root"],
        alias.to_str().unwrap()
    );
    assert_eq!(
        value["Ok"]["result"]["root"]["path"],
        root.path().canonicalize().unwrap().to_str().unwrap()
    );
    for operation in ["copy", "duplicate"] {
        assert!(
            command()
                .args(["files", operation, "--help"])
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    assert!(
        !command()
            .args(["files", "copy-tree", "execute", "--overwrite"])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}
