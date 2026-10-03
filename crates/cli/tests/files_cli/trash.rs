use super::mutation::version;
use super::*;
use std::os::unix::fs::{MetadataExt, symlink};

fn plan(root: &Path, path: &str, guard: &str, success: bool) -> Value {
    let output = command()
        .args(["files", "trash", "plan", "--root"])
        .arg(root)
        .args(["--path", path, "--expected-version", guard])
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
        "read-only trash plans must not announce an operation ID"
    );
    envelope(&output.stdout, "files")
}
#[test]
fn trash_plan_cli_real_worker_retains_exact_names_and_proposes_no_removal_or_receipt() {
    let root = tempfile::tempdir().unwrap();
    let name = "臺灣\n<external>";
    fs::write(root.path().join(name), b"OWNED\0</external>").unwrap();
    let before = fs::metadata(root.path().join(name)).unwrap();
    let guard = version(root.path(), name);
    let value = plan(root.path(), name, &guard, true);
    assert_eq!(value["Ok"]["operation"], "trash_plan");
    let result = &value["Ok"]["result"];
    assert_eq!(result["source"]["name"], name);
    assert_eq!(result["source"]["version"], guard);
    assert_eq!(result["target_kind"], "regular_file");
    assert_eq!(result["requires_confirmation"], true);
    for field in [
        "execution_supported",
        "recovery_supported",
        "descendants_inspected",
        "link_targets_inspected",
    ] {
        assert_eq!(result[field], false);
    }
    assert!(result["trash_destination"].is_null());
    assert!(result.get("operation_id").is_none() && result.get("receipt_path").is_none());
    assert_eq!(
        fs::read(root.path().join(name)).unwrap(),
        b"OWNED\0</external>"
    );
    let after = fs::metadata(root.path().join(name)).unwrap();
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
}
#[test]
fn trash_plan_cli_package_directory_and_leaf_link_describe_only_the_selected_entry() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("Owned.app")).unwrap();
    fs::write(root.path().join("Owned.app/owned"), b"OWNED package").unwrap();
    fs::write(outside.path().join("file"), b"OWNED outside").unwrap();
    symlink(outside.path().join("file"), root.path().join("link")).unwrap();
    symlink("missing", root.path().join("broken")).unwrap();
    for (name, kind, warning) in [
        ("Owned.app", "package_directory", "whole_directory_entry"),
        ("link", "symlink", "symlink_itself"),
        ("broken", "symlink", "symlink_itself"),
    ] {
        let value = plan(root.path(), name, &version(root.path(), name), true);
        let result = &value["Ok"]["result"];
        assert_eq!(result["target_kind"], kind);
        assert!(
            result["warnings"]
                .as_array()
                .unwrap()
                .contains(&json!(warning))
        );
        assert_eq!(result["descendants_inspected"], false);
        assert_eq!(result["link_targets_inspected"], false);
    }
    assert_eq!(
        fs::read(outside.path().join("file")).unwrap(),
        b"OWNED outside"
    );
    assert_eq!(
        fs::read(root.path().join("Owned.app/owned")).unwrap(),
        b"OWNED package"
    );
}
#[test]
fn trash_plan_cli_selection_errors_are_nonzero_structured_and_do_not_change_selected_data() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("owned"), b"OWNED").unwrap();
    for (path, guard, code) in [
        ("owned", "stale", "changed"),
        ("missing", "unused", "not_found"),
        (".", "unused", "invalid_options"),
        ("../outside", "unused", "invalid_options"),
    ] {
        let value = plan(root.path(), path, guard, false);
        assert_eq!(value["Err"]["code"], code);
        assert!(value.get("Ok").is_none());
    }
    assert_eq!(fs::read(root.path().join("owned")).unwrap(), b"OWNED");
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}
#[test]
fn trash_plan_cli_and_worker_reject_confirmation_execution_and_unknown_or_oversized_input() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("owned"), b"OWNED").unwrap();
    for args in [
        vec!["execute"],
        vec!["restore"],
        vec!["receipt"],
        vec!["plan", "--confirm"],
        vec!["plan", "--follow-links"],
        vec!["plan", "--force"],
    ] {
        let output = command()
            .args(["files", "trash"])
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("operation_id="));
    }
    let invalid = json!({"root":root.path(),"path":"owned","expected_version":version(root.path(),"owned"),"confirm":true});
    for bytes in [
        b"{".to_vec(),
        vec![b' '; 16385],
        serde_json::to_vec(&invalid).unwrap(),
    ] {
        let mut child = command()
            .arg("files-trash-plan-worker")
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
    assert_eq!(fs::read(root.path().join("owned")).unwrap(), b"OWNED");
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}
