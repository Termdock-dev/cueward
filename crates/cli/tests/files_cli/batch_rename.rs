use super::mutation::version;
use super::*;
use std::os::unix::fs::MetadataExt;

fn entry(root: &Path, path: &str, name: &str) -> String {
    let parent = Path::new(path)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    json!({
        "path":path, "name":name,
        "expected_version":version(root, path),
        "expected_parent_version":version(root, parent.to_str().unwrap())
    })
    .to_string()
}
fn plan(root: &Path, entries: &[String], success: bool) -> Value {
    let mut command = command();
    command
        .args(["files", "rename-batch", "plan", "--root"])
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
    assert!(
        output.stderr.is_empty(),
        "planning must not announce a mutation receipt"
    );
    envelope(&output.stdout, "files")
}
fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("a"), b"OWNED a\0").unwrap();
    fs::write(root.path().join("b"), b"OWNED b\0").unwrap();
    root
}
#[test]
fn batch_rename_cli_preserves_exact_external_names_versions_and_all_sources() {
    let root = fixture();
    let before = fs::metadata(root.path().join("a")).unwrap();
    let entries = [
        entry(root.path(), "a", "臺灣\n<external>"),
        entry(root.path(), "b", "other"),
    ];
    let value = plan(root.path(), &entries, true);
    assert_eq!(value["Ok"]["operation"], "rename_batch_plan");
    let result = &value["Ok"]["result"];
    assert_eq!(result["execution_supported"], false);
    assert_eq!(result["has_conflicts"], false);
    assert_eq!(result["items"].as_array().unwrap().len(), 2);
    assert_eq!(result["items"][0]["index"], 0);
    assert_eq!(result["request"]["entries"][0]["name"], "臺灣\n<external>");
    assert_eq!(
        result["items"][0]["proposal"]["request"]["destination"],
        "臺灣\n<external>"
    );
    assert_eq!(
        result["items"][0]["proposal"]["source"]["version"],
        serde_json::from_str::<Value>(&entries[0]).unwrap()["expected_version"]
    );
    assert!(result.get("operation_id").is_none());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
    let after = fs::metadata(root.path().join("a")).unwrap();
    assert_eq!(
        (before.ino(), before.mode(), before.mtime(), before.ctime()),
        (after.ino(), after.mode(), after.mtime(), after.ctime())
    );
    assert_eq!(fs::read(root.path().join("a")).unwrap(), b"OWNED a\0");
}
#[test]
fn batch_rename_cli_returns_conflicts_and_stale_item_errors_as_observations() {
    let root = fixture();
    let mut stale: Value = serde_json::from_str(&entry(root.path(), "b", "new")).unwrap();
    stale["expected_version"] = "stale".into();
    let value = plan(
        root.path(),
        &[entry(root.path(), "a", "b"), stale.to_string()],
        true,
    );
    let result = &value["Ok"]["result"];
    assert_eq!(result["has_conflicts"], true);
    assert_eq!(result["issues"][0]["code"], "existing_destination");
    assert_eq!(result["items"][1]["error"]["code"], "changed");
    assert!(result["items"][1]["proposal"].is_null());
    assert_eq!(result["items"][1]["index"], 1);
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
    assert_eq!(fs::read(root.path().join("b")).unwrap(), b"OWNED b\0");
}
#[test]
fn batch_rename_cli_reports_duplicate_targets_case_unicode_collisions_and_swaps() {
    let root = fixture();
    for (names, code) in [
        (["new", "new"], "duplicate_destination"),
        (["Report", "report"], "potential_destination_collision"),
        (["café", "café"], "potential_destination_collision"),
        (["b", "a"], "source_destination_dependency"),
    ] {
        let value = plan(
            root.path(),
            &[
                entry(root.path(), "a", names[0]),
                entry(root.path(), "b", names[1]),
            ],
            true,
        );
        let result = &value["Ok"]["result"];
        assert_eq!(result["has_conflicts"], true);
        assert!(
            result["issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(|issue| issue["code"] == code && issue["entries"] == json!([0, 1]))
        );
    }
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
}
#[test]
fn batch_rename_cli_invalid_entry_json_and_serialized_budget_fail_before_worker() {
    let root = fixture();
    for entries in [
        vec!["{}".into()],
        vec!["not JSON".into()],
        vec!["{}".into(); 65],
        vec!["x".repeat(16385)],
    ] {
        assert_eq!(
            plan(root.path(), &entries, false)["Err"]["code"],
            "invalid_options"
        );
    }
    let entries = vec![entry(root.path(), "a", &"x".repeat(255)); 64];
    assert_eq!(
        plan(root.path(), &entries, false)["Err"]["code"],
        "invalid_options"
    );
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
}
#[test]
fn batch_rename_cli_all_invalid_paths_keep_every_index_without_silent_success() {
    let root = fixture();
    let entries = ["../outside", "missing", "/absolute"].map(|path| {
        json!({"path":path,"name":"new","expected_version":"s","expected_parent_version":"p"})
            .to_string()
    });
    let value = plan(root.path(), &entries, true);
    let result = &value["Ok"]["result"];
    assert_eq!(result["has_conflicts"], true);
    assert_eq!(result["execution_supported"], false);
    for (index, item) in result["items"].as_array().unwrap().iter().enumerate() {
        assert_eq!(item["index"], index);
        assert!(item["proposal"].is_null() && item["error"].is_object());
    }
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
}
