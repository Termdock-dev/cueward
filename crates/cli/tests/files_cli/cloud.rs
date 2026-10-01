use super::*;

fn cloud(root: &Path, action: &str, args: &[&str], success: bool) -> Value {
    let output = command()
        .args(["files", "cloud", action, "--root"])
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert_eq!(
        output.status.success(),
        success,
        "stdout: {}, stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    envelope(&output.stdout, "files")
}

#[test]
fn local_cloud_status_preserves_external_paths_and_non_icloud_field_semantics() {
    let dir = tempfile::tempdir().unwrap();
    let name = "臺灣\n <external>.txt";
    fs::write(dir.path().join(name), "unchanged").unwrap();
    let output = cloud(dir.path(), "status", &["--path", name], true);
    assert_eq!(output["Ok"]["operation"], "cloud_status");
    let value = &output["Ok"]["result"];
    assert_eq!(value["file"]["name"], name);
    assert_eq!(
        value["provider_coverage"],
        "icloud_keys_other_providers_unknown"
    );
    assert_eq!(value["download_requested_by_operation"], false);
    let membership = &value["resources"]["is_ubiquitous"];
    let dependent_status = match membership["status"].as_str().unwrap() {
        "available" => {
            assert_eq!(membership["value"], false);
            "not_applicable"
        }
        "unavailable" | "error" => {
            assert!(membership.get("value").is_none());
            "unavailable"
        }
        status => panic!("unexpected local membership status: {status}"),
    };
    assert_eq!(
        value["resources"]["downloading_status"]["status"],
        dependent_status
    );
    assert_eq!(fs::read(dir.path().join(name)).unwrap(), b"unchanged");
}

#[test]
fn local_download_version_scope_and_membership_rejections_leave_data_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a"), "unchanged").unwrap();
    let output = cloud(dir.path(), "status", &["--path", "a"], true);
    let version = output["Ok"]["result"]["file"]["version"].as_str().unwrap();
    for (path, expected, code) in [
        ("a", version, "unsupported_type"),
        ("a", "stale", "changed"),
        ("../outside", version, "outside_root"),
    ] {
        assert_eq!(
            cloud(
                dir.path(),
                "download",
                &["--path", path, "--expected-version", expected],
                false
            )["Err"]["code"],
            code
        );
    }
    assert_eq!(fs::read(dir.path().join("a")).unwrap(), b"unchanged");
}

#[test]
fn cloud_leaf_symlink_fields_remain_not_applicable_without_resolving_target() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    symlink("/outside/missing", dir.path().join("link")).unwrap();
    let output = cloud(dir.path(), "status", &["--path", "link"], true);
    assert_eq!(output["Ok"]["result"]["file"]["kind"], "symlink");
    assert_eq!(
        output["Ok"]["result"]["resources"]["is_ubiquitous"]["status"],
        "not_applicable"
    );
}

#[test]
fn cloud_worker_rejects_malformed_oversized_and_unguarded_download_requests() {
    let dir = tempfile::tempdir().unwrap();
    for bytes in [b"{".to_vec(), vec![b' '; 16385], serde_json::to_vec(&json!({"root":dir.path(),"path":"a","follow_links":false,"expected_version":null,"action":{"operation":"download","max_bytes":1}})).unwrap()] {
        let mut child = command().arg("files-cloud-worker").stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
        child.stdin.take().unwrap().write_all(&bytes).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(!output.status.success());
        assert_eq!(envelope(&output.stdout, "files/worker")["Err"]["code"], "invalid_options");
    }
}
