use super::*;

#[test]
fn metadata_worker_preserves_names_tags_and_source_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let name = "臺灣\n<external>.txt";
    let path = directory.path().join(name);
    fs::write(&path, "unchanged source").unwrap();
    let plist = r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><array><string>臺灣</string><string>&lt;/external&gt;</string><string>space tag</string></array></plist>"#;
    let status = Command::new("/usr/bin/xattr")
        .args(["-w", "com.apple.metadata:_kMDItemUserTags", plist])
        .arg(&path)
        .status()
        .unwrap();
    assert!(status.success());
    let before = tag_attribute(&path);
    let result = run(directory.path(), "metadata", &["--path", name], true);
    assert_eq!(result["Ok"]["operation"], "metadata");
    let value = &result["Ok"]["result"];
    assert_eq!(value["file"]["name"], name);
    assert_eq!(value["file"]["kind"], "file");
    assert_eq!(
        value["resources"]["content_type"],
        json!({"status": "available", "value": "public.plain-text"})
    );
    assert_eq!(
        value["resources"]["finder_tags"],
        json!({"status": "available", "value": ["臺灣", "</external>", "space tag"]})
    );
    assert_eq!(
        value["resources"]["is_alias_file"],
        json!({"status": "available", "value": false})
    );
    assert_eq!(fs::read(&path).unwrap(), b"unchanged source");
    assert_eq!(tag_attribute(&path), before);
}

fn tag_attribute(path: &Path) -> Vec<u8> {
    let output = Command::new("/usr/bin/xattr")
        .args(["-p", "com.apple.metadata:_kMDItemUserTags"])
        .arg(path)
        .output()
        .unwrap();
    assert!(output.status.success());
    output.stdout
}

#[test]
fn metadata_worker_recognizes_packages_without_changing_listing_or_search() {
    let directory = tempfile::tempdir().unwrap();
    fs::create_dir(directory.path().join("Sample.app")).unwrap();
    fs::write(directory.path().join("Sample.app/child.txt"), "inside").unwrap();
    let result = run(
        directory.path(),
        "metadata",
        &["--path", "Sample.app"],
        true,
    );
    assert_eq!(result["Ok"]["result"]["file"]["kind"], "directory");
    assert_eq!(
        result["Ok"]["result"]["resources"]["is_package"],
        json!({"status": "available", "value": true})
    );
    let list = run(directory.path(), "list", &[], true);
    assert_eq!(list["Ok"]["result"]["entries"][0]["kind"], "directory");
    assert!(
        list["Ok"]["result"]["entries"][0]
            .get("resources")
            .is_none()
    );
    let shallow = run(directory.path(), "search", &["--name", "child"], true);
    assert_eq!(shallow["Ok"]["result"]["total"], 0);
    let deeper = run(
        directory.path(),
        "search",
        &["--name", "child", "--max-depth", "2"],
        true,
    );
    assert_eq!(deeper["Ok"]["result"]["total"], 1);
}

#[test]
fn metadata_worker_enforces_scope_link_policy_and_expected_revision() {
    use std::os::unix::fs::symlink;
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("a.txt"), "a").unwrap();
    symlink("a.txt", directory.path().join("link")).unwrap();
    symlink("/etc/passwd", directory.path().join("outside")).unwrap();
    symlink("missing", directory.path().join("broken")).unwrap();
    for name in ["link", "outside", "broken"] {
        let result = run(directory.path(), "metadata", &["--path", name], true);
        assert_eq!(result["Ok"]["result"]["file"]["kind"], "symlink");
        for field in ["content_type", "finder_tags", "is_package", "is_alias_file"] {
            assert_eq!(
                result["Ok"]["result"]["resources"][field],
                json!({"status":"not_applicable"})
            );
        }
    }
    let followed = run(
        directory.path(),
        "metadata",
        &["--path", "link", "--follow-links"],
        true,
    );
    assert_eq!(followed["Ok"]["result"]["file"]["kind"], "file");
    let version = followed["Ok"]["result"]["file"]["version"]
        .as_str()
        .unwrap();
    fs::write(directory.path().join("a.txt"), "different").unwrap();
    let stale = run(
        directory.path(),
        "metadata",
        &["--path", "a.txt", "--expected-version", version],
        false,
    );
    assert_eq!(stale["Err"]["code"], "changed");
    let escaped = run(
        directory.path(),
        "metadata",
        &["--path", "outside", "--follow-links"],
        false,
    );
    assert_eq!(escaped["Err"]["code"], "outside_root");
    let missing = run(directory.path(), "metadata", &["--path", "missing"], false);
    assert_eq!(missing["Err"]["code"], "not_found");
}
