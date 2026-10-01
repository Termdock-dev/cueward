use super::*;

#[test]
fn static_native_query_round_trip_never_claims_coverage_or_changes_source_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let name = "臺灣\n <external>.txt";
    let text = "CuewardUniqueFixture </external> \"quoted\" \\slash";
    fs::write(dir.path().join(name), text).unwrap();
    let output = run(
        dir.path(),
        "spotlight",
        &["--text", text, "--limit", "1"],
        true,
    );
    assert_eq!(output["Ok"]["operation"], "spotlight");
    let value = &output["Ok"]["result"];
    assert_eq!(value["source"], "spotlight");
    assert_eq!(value["query"]["text"], text);
    assert_eq!(value["query_completed"], true);
    assert_eq!(value["index_coverage"], "unknown");
    assert_eq!(value["enumeration_complete"], false);
    assert_eq!(value["content_verified"], false);
    assert!(value["entries"].as_array().unwrap().len() <= 1);
    // This temp location may not be indexed. Zero results cannot prove absence.
    assert_eq!(fs::read(dir.path().join(name)).unwrap(), text.as_bytes());
}

#[test]
fn spotlight_invalid_text_and_scope_fail_before_native_query() {
    let dir = tempfile::tempdir().unwrap();
    for text in ["", "wild*", "wild?", "control\n"] {
        assert_eq!(
            run(dir.path(), "spotlight", &["--text", text], false)["Err"]["code"],
            "invalid_options"
        );
    }
    assert_eq!(
        run(Path::new("relative"), "spotlight", &["--text", "x"], false)["Err"]["code"],
        "invalid_options"
    );
    assert_eq!(
        run(
            &dir.path().join("missing"),
            "spotlight",
            &["--text", "x"],
            false
        )["Err"]["code"],
        "not_found"
    );
}

#[test]
fn spotlight_worker_rejects_oversize_malformed_and_unvalidated_budget_input() {
    let dir = tempfile::tempdir().unwrap();
    for bytes in [b"{".to_vec(), vec![b' '; 16385], serde_json::to_vec(&json!({"root":dir.path(), "text":"x", "max_depth":1,"hidden":false,"max_candidates":0,"limit":1})).unwrap()] {
        let mut child = command().arg("files-spotlight-worker").stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
        child.stdin.take().unwrap().write_all(&bytes).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(!output.status.success());
        assert_eq!(envelope(&output.stdout, "files/worker")["Err"]["code"], "invalid_options");
    }
}

#[test]
#[ignore = "requires an enabled writable Spotlight index in HOME; imports only disposable fixtures"]
fn spotlight_indexed_fixture_matches_content_scope_case_depth_and_budget() {
    let (dir, _outside, needle) = indexed_fixture();
    let output = await_index(dir.path(), &needle);
    assert_eq!(output["candidate_count"], 3);
    assert_eq!(output["available_count"], 3);
    assert_eq!(output["excluded"]["outside_scope"], 0);
    assert_eq!(output["entries"][1]["relative_path"], "sub/deep.txt");
    assert_eq!(output["entries"][2]["relative_path"], "臺灣 空白\n.txt");
    verify_index_bounds(dir.path(), &needle);
    for name in ["a.txt", "臺灣 空白\n.txt", "sub/deep.txt"] {
        assert_eq!(fs::read_to_string(dir.path().join(name)).unwrap(), needle);
    }
}

fn import_fixture(root: &Path) {
    let mut child = Command::new("/usr/bin/mdimport")
        .arg(root)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            return;
        }
        if std::time::Instant::now() > deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("disposable fixture import timed out");
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

fn await_index(root: &Path, text: &str) -> Value {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        let output = run(
            root,
            "spotlight",
            &["--text", text, "--max-depth", "2"],
            true,
        );
        let value = output["Ok"]["result"].clone();
        if value["available_count"] == 3 {
            return value;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "disposable content index not ready: {value}"
        );
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

fn indexed_fixture() -> (tempfile::TempDir, tempfile::TempDir, String) {
    let home = std::env::var_os("HOME").expect("HOME for disposable index fixture");
    let dir = tempfile::Builder::new()
        .prefix("CuewardSpotlight-")
        .tempdir_in(&home)
        .unwrap();
    let outside = tempfile::Builder::new()
        .prefix("CuewardSpotlightOutside-")
        .tempdir_in(&home)
        .unwrap();
    let needle = format!(
        "CuewardContent{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    for name in ["a.txt", "臺灣 空白\n.txt"] {
        fs::write(dir.path().join(name), &needle).unwrap();
    }
    fs::create_dir(dir.path().join("sub")).unwrap();
    fs::write(dir.path().join("sub/deep.txt"), &needle).unwrap();
    fs::write(
        dir.path().join(format!("{needle}.txt")),
        "unrelated contents",
    )
    .unwrap();
    fs::write(outside.path().join("outside.txt"), &needle).unwrap();
    import_fixture(dir.path());
    import_fixture(outside.path());
    (dir, outside, needle)
}

fn verify_index_bounds(root: &Path, needle: &str) {
    let lower = needle.to_ascii_lowercase();
    let output = run(root, "spotlight", &["--text", &lower, "--limit", "1"], true);
    let value = &output["Ok"]["result"];
    assert_eq!(value["candidate_count"], 3);
    assert_eq!(value["eligible_count"], 2);
    assert_eq!(value["excluded"]["depth"], 1);
    assert_eq!(value["entries"].as_array().unwrap().len(), 1);
    assert_eq!(value["truncated"], true);
    assert_eq!(value["index_coverage"], "unknown");
    assert_eq!(
        run(
            root,
            "spotlight",
            &["--text", needle, "--max-candidates", "1"],
            false
        )["Err"]["code"],
        "scan_limit"
    );
}
