use super::mutation::{cleanup, version};
use super::*;
use std::os::unix::fs::{MetadataExt, symlink};
fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("Owned.app/Contents/Nested.app")).unwrap();
    fs::write(
        root.path().join("Owned.app/Contents/臺灣\n<external>"),
        b"OWNED package\0</external>",
    )
    .unwrap();
    fs::write(
        root.path().join("Owned.app/Contents/Nested.app/.hidden"),
        b"OWNED nested",
    )
    .unwrap();
    root
}
fn copy(root: &Path, action: &str, include: bool, extra: &[&str]) -> std::process::Output {
    let mut cli = command();
    cli.args(["files", "copy-tree", action, "--root"])
        .arg(root)
        .args([
            "--path",
            "Owned.app",
            "--destination",
            "副本\n<external>.app",
            "--expected-version",
            &version(root, "Owned.app"),
            "--expected-parent-version",
            &version(root, "."),
        ]);
    if include {
        cli.arg("--include-packages");
    }
    cli.args(extra).output().unwrap()
}
#[test]
fn package_cli_plan_defaults_to_skip_and_explicit_inclusion_is_still_read_only() {
    let root = fixture();
    let before = version(root.path(), "Owned.app/Contents/臺灣\n<external>");
    for include in [false, true] {
        let output = copy(root.path(), "plan", include, &[]);
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let result = envelope(&output.stdout, "files");
        let plan = &result["Ok"]["result"];
        assert_eq!(result["Ok"]["operation"], "copy_tree_plan");
        assert_eq!(plan["enumeration_complete"], include);
        assert_eq!(plan["has_blockers"], !include);
        assert_eq!(plan["execution_supported"], false);
        assert_eq!(
            plan["entries"].as_array().unwrap().len(),
            if include { 5 } else { 1 }
        );
        if include {
            assert_eq!(plan["request"]["include_packages"], true);
        } else {
            assert!(plan["request"].get("include_packages").is_none());
        }
        assert!(!root.path().join("副本\n<external>.app").exists());
    }
    assert_eq!(
        version(root.path(), "Owned.app/Contents/臺灣\n<external>"),
        before
    );
}
#[test]
fn package_cli_actual_worker_publishes_verified_independent_package_with_saved_opt_in() {
    let root = fixture();
    let source = root.path().join("Owned.app/Contents/臺灣\n<external>");
    assert!(
        Command::new("/usr/bin/xattr")
            .args(["-w", "com.cueward.owned-package-cli", "OWNED attributes"])
            .arg(&source)
            .status()
            .unwrap()
            .success()
    );
    let before = version(root.path(), "Owned.app/Contents/臺灣\n<external>");
    let output = copy(root.path(), "execute", true, &[]);
    let result = envelope(&output.stdout, "files");
    let receipt = &result["Ok"]["result"];
    assert!(output.status.success(), "{receipt}");
    assert_eq!(receipt["status"], "completed");
    assert_eq!(receipt["completion_verified"], true);
    assert_eq!(receipt["request"]["include_packages"], true);
    let target = root
        .path()
        .join("副本\n<external>.app/Contents/臺灣\n<external>");
    assert_eq!(fs::read(&source).unwrap(), fs::read(&target).unwrap());
    assert_ne!(
        fs::metadata(&source).unwrap().ino(),
        fs::metadata(&target).unwrap().ino()
    );
    assert_eq!(
        fs::metadata(&source).unwrap().modified().unwrap(),
        fs::metadata(&target).unwrap().modified().unwrap()
    );
    let attr = Command::new("/usr/bin/xattr")
        .args(["-p", "com.cueward.owned-package-cli"])
        .arg(&target)
        .output()
        .unwrap();
    assert!(attr.status.success());
    assert_eq!(
        String::from_utf8(attr.stdout).unwrap().trim_end(),
        "OWNED attributes"
    );
    assert_eq!(
        version(root.path(), "Owned.app/Contents/臺灣\n<external>"),
        before
    );
    assert!(
        fs::read(
            root.path()
                .join("副本\n<external>.app/Contents/Nested.app/.hidden")
        )
        .is_ok()
    );
    let id = receipt["operation_id"].as_str().unwrap();
    assert!(String::from_utf8_lossy(&output.stderr).contains(id));
    let saved = command()
        .args(["files", "copy-tree", "receipt", "--operation-id", id])
        .output()
        .unwrap();
    assert!(saved.status.success());
    assert_eq!(envelope(&saved.stdout, "files")["Ok"]["result"], *receipt);
    cleanup(receipt);
}
#[test]
fn package_cli_no_opt_in_links_conflicts_or_limits_refuse_before_staging() {
    for mode in 0..4 {
        let root = fixture();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("PRIVATE"), b"not selected").unwrap();
        if mode == 1 {
            symlink(outside.path(), root.path().join("Owned.app/outside")).unwrap();
        }
        if mode == 2 {
            fs::write(root.path().join("副本\n<external>.app"), b"OWNED conflict").unwrap();
        }
        let output = copy(
            root.path(),
            "execute",
            mode != 0,
            if mode == 3 {
                &["--max-entries", "1"]
            } else {
                &[]
            },
        );
        assert!(!output.status.success());
        let result = envelope(&output.stdout, "files");
        let receipt = &result["Ok"]["result"];
        assert_eq!(receipt["status"], "not_started");
        assert!(receipt["staging_path"].is_null());
        assert_eq!(receipt["mutation_attempted"], false);
        assert_eq!(
            fs::read(root.path().join("Owned.app/Contents/臺灣\n<external>")).unwrap(),
            b"OWNED package\0</external>"
        );
        if mode == 2 {
            assert_eq!(
                fs::read(root.path().join("副本\n<external>.app")).unwrap(),
                b"OWNED conflict"
            );
        } else {
            assert!(!root.path().join("副本\n<external>.app").exists());
        }
        cleanup(receipt);
    }
}
