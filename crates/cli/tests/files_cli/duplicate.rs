use super::mutation::{cleanup, verify_cannot_replay, verify_saved_receipt, version};
use super::*;
use std::os::unix::fs::MetadataExt;

fn invoke(root: &Path, source: &str, name: &str, success: bool) -> Value {
    let parent = Path::new(source)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let source_version = version(root, source);
    let parent_version = version(root, parent.to_str().unwrap());
    run(
        root,
        "duplicate",
        &[
            "--path",
            source,
            "--name",
            name,
            "--expected-version",
            &source_version,
            "--expected-parent-version",
            &parent_version,
        ],
        success,
    )
}
#[test]
fn duplicate_cli_creates_verified_sibling_and_preserves_source_and_receipt_identity() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("nested")).unwrap();
    let source = "nested/來源\n<external>";
    let name = "副本\n<external>";
    let content = b"OWNED CLI duplicate\0</external>";
    fs::write(root.path().join(source), content).unwrap();
    let before = version(root.path(), source);
    let value = invoke(root.path(), source, name, true);
    assert_eq!(value["Ok"]["operation"], "duplicate");
    let receipt = &value["Ok"]["result"];
    assert_eq!(receipt["request"]["action"]["operation"], "duplicate");
    assert_eq!(receipt["request"]["destination"], format!("nested/{name}"));
    assert_eq!(receipt["status"], "completed");
    assert_eq!(receipt["completion_verified"], true);
    assert_eq!(
        receipt["verification"]["content_sha256"],
        super::preview::digest(content)
    );
    let destination = root.path().join("nested").join(name);
    assert_eq!(fs::read(&destination).unwrap(), content);
    assert_ne!(
        destination.metadata().unwrap().ino(),
        root.path().join(source).metadata().unwrap().ino()
    );
    assert_eq!(version(root.path(), source), before);
    verify_saved_receipt(receipt);
    verify_cannot_replay(receipt);
    fs::write(destination, b"OWNED independent edit").unwrap();
    assert_eq!(fs::read(root.path().join(source)).unwrap(), content);
    cleanup(receipt);
}
#[test]
fn duplicate_cli_conflicts_exit_nonzero_and_never_replace_or_auto_rename() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("source"), b"OWNED source").unwrap();
    fs::write(root.path().join("existing"), b"OWNED destination").unwrap();
    symlink("missing-owned-target", root.path().join("link")).unwrap();
    for name in ["existing", "source", "link"] {
        let result = invoke(root.path(), "source", name, false);
        assert_eq!(result["Ok"]["result"]["error"]["code"], "conflict");
        assert_eq!(result["Ok"]["result"]["status"], "not_started");
        cleanup(&result["Ok"]["result"]);
    }
    assert_eq!(
        fs::read(root.path().join("existing")).unwrap(),
        b"OWNED destination"
    );
    assert_eq!(
        fs::read(root.path().join("source")).unwrap(),
        b"OWNED source"
    );
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 3);
}
#[test]
fn duplicate_cli_invalid_names_fail_without_receipt_announcement_or_a_new_file() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("source"), b"OWNED source").unwrap();
    for name in ["", ".", "..", "other/name", "name/", "/absolute"] {
        let output = command()
            .args(["files", "duplicate", "--root"])
            .arg(root.path())
            .args([
                "--path",
                "source",
                "--name",
                name,
                "--expected-version",
                "s",
                "--expected-parent-version",
                "p",
            ])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert_eq!(
            envelope(&output.stdout, "files")["Err"]["code"],
            "invalid_options"
        );
        assert!(output.stderr.is_empty());
    }
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}
#[test]
fn duplicate_cli_copies_packages_but_stale_source_is_a_failed_receipt() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("Owned.app")).unwrap();
    fs::write(root.path().join("source"), b"OWNED source").unwrap();
    let directory = invoke(root.path(), "Owned.app", "directory-copy", true);
    assert_eq!(directory["Ok"]["result"]["status"], "completed");
    cleanup(&directory["Ok"]["result"]);
    let parent = version(root.path(), ".");
    let stale = run(
        root.path(),
        "duplicate",
        &[
            "--path",
            "source",
            "--name",
            "new",
            "--expected-version",
            "stale",
            "--expected-parent-version",
            &parent,
        ],
        false,
    );
    assert_eq!(stale["Ok"]["result"]["status"], "not_started");
    assert_eq!(stale["Ok"]["result"]["error"]["code"], "changed");
    cleanup(&stale["Ok"]["result"]);
    assert!(!root.path().join("new").exists());
    assert!(root.path().join("directory-copy").is_dir());
}
