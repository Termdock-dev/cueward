//! Actual CLI/worker link-object relocation, never target relocation or reference repair.
use super::mutation::{cleanup, version};
use super::*;
use std::os::unix::fs::{MetadataExt, symlink};
fn invoke(
    root: &Path,
    action: &str,
    path: &str,
    target: &str,
    parent: &str,
    extra: &[&str],
    success: bool,
) -> Value {
    let sv = version(root, path);
    let pv = version(root, parent);
    let flag = if action == "rename" {
        "--name"
    } else {
        "--destination"
    };
    run(
        root,
        action,
        &[
            &[
                "--path",
                path,
                flag,
                target,
                "--expected-version",
                &sv,
                "--expected-parent-version",
                &pv,
            ],
            extra,
        ]
        .concat(),
        success,
    )["Ok"]["result"]
        .clone()
}
#[test]
fn link_cli_plan_move_rename_noop_and_saved_receipt_preserve_inode_reference_and_outside_target() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().join("PRIVATE");
    fs::write(&target, b"OWNED outside").unwrap();
    fs::create_dir(root.path().join("to")).unwrap();
    let name = "臺灣\n<external>";
    symlink(&target, root.path().join(name)).unwrap();
    let before = version(outside.path(), "PRIVATE");
    let link_version = version(root.path(), name);
    let inode = fs::symlink_metadata(root.path().join(name)).unwrap().ino();
    let plan = invoke(
        root.path(),
        "move",
        name,
        "to/link",
        "to",
        &["--link-itself", "--dry-run"],
        true,
    );
    assert_eq!(plan["request"]["link_itself"], true);
    assert_eq!(plan["source"]["kind"], "symlink");
    assert_eq!(
        plan["source_resources"]["content_type"]["status"],
        "not_applicable"
    );
    assert_eq!(version(root.path(), name), link_version);
    let mut source = name;
    for (action, next, parent) in [
        ("move", "to/link", "to"),
        ("rename", "renamed", "to"),
        ("rename", "renamed", "to"),
    ] {
        let receipt = invoke(
            root.path(),
            action,
            source,
            next,
            parent,
            &["--link-itself"],
            true,
        );
        assert_eq!(receipt["status"], "completed");
        assert_eq!(receipt["completion_verified"], true);
        assert_eq!(receipt["request"]["link_itself"], true);
        let destination = receipt["destination_after"]["path"].as_str().unwrap();
        assert_eq!(fs::read_link(destination).unwrap(), target);
        assert_eq!(fs::symlink_metadata(destination).unwrap().ino(), inode);
        let output = command()
            .args([
                "files",
                "relocation",
                "receipt",
                "--operation-id",
                receipt["operation_id"].as_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(envelope(&output.stdout, "files")["Ok"]["result"], receipt);
        cleanup(&receipt);
        source = if action == "move" {
            "to/link"
        } else {
            "to/renamed"
        };
    }
    assert_eq!(version(outside.path(), "PRIVATE"), before);
    assert_eq!(fs::read(&target).unwrap(), b"OWNED outside");
}
#[test]
fn link_cli_handles_broken_relative_targets_without_rewriting_them_and_default_remains_blocked() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("to")).unwrap();
    symlink("../missing\n<external>", root.path().join("link")).unwrap();
    let blocked = invoke(root.path(), "move", "link", "to/link", "to", &[], false);
    assert_eq!(blocked["status"], "not_started");
    assert_eq!(blocked["mutation_attempted"], false);
    cleanup(&blocked);
    let receipt = invoke(
        root.path(),
        "move",
        "link",
        "to/link",
        "to",
        &["--link-itself"],
        true,
    );
    assert_eq!(
        receipt["destination_after"]["link_target"],
        "../missing\n<external>"
    );
    assert_eq!(
        fs::read_link(root.path().join("to/link")).unwrap(),
        Path::new("../missing\n<external>")
    );
    assert!(fs::symlink_metadata(root.path().join("link")).is_err());
    cleanup(&receipt);
}
#[test]
fn link_cli_conflicts_and_nonlink_opt_in_refuse_without_changing_selected_data() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("owned"), b"OWNED unchanged").unwrap();
    symlink("missing", root.path().join("link")).unwrap();
    symlink("existing", root.path().join("conflict")).unwrap();
    for (source, destination) in [("link", "conflict"), ("owned", "new")] {
        let r = invoke(
            root.path(),
            "move",
            source,
            destination,
            ".",
            &["--link-itself"],
            false,
        );
        assert_eq!(r["status"], "not_started");
        assert_eq!(r["mutation_attempted"], false);
        cleanup(&r);
    }
    assert_eq!(
        fs::read(root.path().join("owned")).unwrap(),
        b"OWNED unchanged"
    );
    assert_eq!(
        fs::read_link(root.path().join("link")).unwrap(),
        Path::new("missing")
    );
    assert_eq!(
        fs::read_link(root.path().join("conflict")).unwrap(),
        Path::new("existing")
    );
    assert!(fs::symlink_metadata(root.path().join("new")).is_err());
}
