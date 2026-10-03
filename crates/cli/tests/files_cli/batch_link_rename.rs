//! Actual CLI/worker selection never follows a link target or grants batch-wide permission.
use super::batch_execution::{cleanup, execute};
use super::batch_rename::{entry, fixture};
use super::mutation::version;
use super::*;
use std::os::unix::fs::{MetadataExt, symlink};

fn link_entry(root: &Path, path: &str, name: &str) -> String {
    let mut value: Value = serde_json::from_str(&entry(root, path, name)).unwrap();
    value["link_itself"] = true.into();
    value.to_string()
}
fn lookup(operation: &str, id: &str) -> Value {
    let output = command()
        .args(["files", operation, "receipt", "--operation-id", id])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    envelope(&output.stdout, "files")["Ok"]["result"].clone()
}
#[test]
fn batch_link_cli_verifies_mixed_sources_nested_parents_noops_and_saved_permission() {
    let root = fixture();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("PRIVATE"), b"OWNED outside").unwrap();
    let outside_before = version(outside.path(), "PRIVATE");
    symlink("missing\n<external>", root.path().join("broken")).unwrap();
    fs::create_dir(root.path().join("nested")).unwrap();
    symlink("../a", root.path().join("nested/link")).unwrap();
    symlink(outside.path().join("PRIVATE"), root.path().join("outside")).unwrap();
    let paths = ["broken", "a", "nested/link", "outside"];
    let inodes: Vec<_> = paths
        .iter()
        .map(|p| fs::symlink_metadata(root.path().join(p)).unwrap().ino())
        .collect();
    let entries = [
        link_entry(root.path(), "broken", "臺灣\n<external>"),
        entry(root.path(), "a", "ordinary"),
        link_entry(root.path(), "nested/link", "new"),
        link_entry(root.path(), "outside", "outside"),
    ];
    let value = execute(root.path(), &entries, true);
    let receipt = &value["Ok"]["result"];
    assert_eq!(receipt["status"], "completed");
    assert_eq!(receipt["completion_verified"], true);
    assert_eq!(
        lookup("rename-batch", receipt["operation_id"].as_str().unwrap()),
        *receipt
    );
    for (index, item) in receipt["items"].as_array().unwrap().iter().enumerate() {
        let child = lookup("relocation", item["operation_id"].as_str().unwrap());
        assert_eq!(child["completion_verified"], true);
        let after = &child["destination_after"];
        assert_eq!(after["identity"], child["before"]["source"]["identity"]);
        assert_eq!(
            after["link_target"],
            child["before"]["source"]["link_target"]
        );
        assert_eq!(
            fs::symlink_metadata(after["path"].as_str().unwrap())
                .unwrap()
                .ino(),
            inodes[index]
        );
        if index != 1 {
            assert_eq!(child["request"]["link_itself"], true);
            assert_eq!(receipt["request"]["entries"][index]["link_itself"], true);
        } else {
            assert!(child["request"].get("link_itself").is_none());
            assert!(
                receipt["request"]["entries"][index]
                    .get("link_itself")
                    .is_none()
            );
        }
        if index == 3 {
            assert_eq!(child["renamed"], false);
        }
    }
    assert_eq!(
        fs::read_link(root.path().join("臺灣\n<external>")).unwrap(),
        Path::new("missing\n<external>")
    );
    assert_eq!(
        fs::read_link(root.path().join("nested/new")).unwrap(),
        Path::new("../a")
    );
    assert_eq!(
        fs::read(root.path().join("ordinary")).unwrap(),
        b"OWNED a\0"
    );
    assert_eq!(version(outside.path(), "PRIVATE"), outside_before);
    assert_eq!(
        fs::read(outside.path().join("PRIVATE")).unwrap(),
        b"OWNED outside"
    );
    cleanup(receipt);
}
#[test]
fn batch_link_cli_preflight_refuses_missing_permission_nonlinks_and_existing_destinations() {
    for mode in 0..3 {
        let root = fixture();
        symlink("missing", root.path().join("link")).unwrap();
        let entries = match mode {
            0 => [
                entry(root.path(), "a", "new"),
                entry(root.path(), "link", "other"),
            ],
            1 => [
                link_entry(root.path(), "a", "new"),
                link_entry(root.path(), "link", "other"),
            ],
            _ => [
                entry(root.path(), "a", "new"),
                link_entry(root.path(), "link", "b"),
            ],
        };
        let value = execute(root.path(), &entries, false);
        let receipt = &value["Ok"]["result"];
        assert_eq!(receipt["status"], "not_started");
        assert!(
            receipt["items"]
                .as_array()
                .unwrap()
                .iter()
                .all(|i| i["operation_id"].is_null() && i["mutation_attempted"] == false)
        );
        assert_eq!(fs::read(root.path().join("a")).unwrap(), b"OWNED a\0");
        assert_eq!(fs::read(root.path().join("b")).unwrap(), b"OWNED b\0");
        assert_eq!(
            fs::read_link(root.path().join("link")).unwrap(),
            Path::new("missing")
        );
        assert!(fs::symlink_metadata(root.path().join("new")).is_err());
        assert_eq!(
            lookup("rename-batch", receipt["operation_id"].as_str().unwrap()),
            *receipt
        );
        cleanup(receipt);
    }
}
