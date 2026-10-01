use super::*;

#[test]
fn search_worker_preserves_exact_names_and_query_scope_with_real_pagination() {
    let directory = tempfile::tempdir().unwrap();
    fs::create_dir(directory.path().join("Reports")).unwrap();
    let first_name = "Reports/a臺灣\n<external>.txt";
    let second_name = "Reports/b臺灣.txt";
    fs::write(directory.path().join(first_name), "123").unwrap();
    fs::write(directory.path().join(second_name), "456").unwrap();
    fs::write(directory.path().join("Reports/not-match.txt"), "789").unwrap();
    fs::write(directory.path().join("top臺灣.txt"), "00").unwrap();
    let common = [
        "--path",
        "Reports",
        "--name",
        "臺灣",
        "--kind",
        "file",
        "--min-size",
        "3",
        "--max-size",
        "3",
        "--max-depth",
        "2",
    ];
    let mut args = common.to_vec();
    args.extend(["--limit", "1"]);
    let first = run(directory.path(), "search", &args, true);
    let value = &first["Ok"]["result"];
    assert_eq!(first["Ok"]["operation"], "search");
    assert_eq!(value["source"], "filesystem");
    assert_eq!(value["query"]["name"], "臺灣");
    assert_eq!(value["query"]["max_depth"], 2);
    assert_eq!(value["entries"][0]["relative_path"], first_name);
    assert_eq!(value["entries"][0]["file"]["name"], "a臺灣\n<external>.txt");
    assert_eq!(value["total"], 2);
    assert_eq!(value["next_offset"], 1);
    assert_eq!(value["enumeration_complete"], true);
    assert_eq!(value["entries_observed"], 3);
    let token = value["version"].as_str().unwrap();
    let mut args = common.to_vec();
    args.extend(["--limit", "2", "--offset", "1", "--expected-version", token]);
    let second = run(directory.path(), "search", &args, true);
    assert_eq!(
        second["Ok"]["result"]["entries"][0]["relative_path"],
        second_name
    );
    assert!(second["Ok"]["result"]["next_offset"].is_null());
    fs::write(directory.path().join("Reports/not-match.txt"), "changed").unwrap();
    let stale = run(directory.path(), "search", &args, false);
    assert_eq!(stale["Err"]["code"], "changed");
    assert_eq!(fs::read(directory.path().join(first_name)).unwrap(), b"123");
    assert_eq!(
        fs::read(directory.path().join(second_name)).unwrap(),
        b"456"
    );
}

#[test]
fn search_zero_matches_and_incomplete_scan_are_distinct() {
    let directory = tempfile::tempdir().unwrap();
    fs::create_dir(directory.path().join("dir")).unwrap();
    fs::write(directory.path().join(".hidden"), "").unwrap();
    fs::write(directory.path().join("dir/a"), "").unwrap();
    let no_match = run(
        directory.path(),
        "search",
        &["--name", "not present", "--max-depth", "2"],
        true,
    );
    assert_eq!(no_match["Ok"]["result"]["total"], 0);
    assert_eq!(no_match["Ok"]["result"]["enumeration_complete"], true);
    assert_eq!(no_match["Ok"]["result"]["entries_observed"], 3);
    let limited = run(
        directory.path(),
        "search",
        &[
            "--name",
            "not present",
            "--max-depth",
            "2",
            "--max-entries",
            "2",
        ],
        false,
    );
    assert_eq!(limited["Err"]["code"], "scan_limit");
    assert!(limited.get("Ok").is_none());
    let missing = run(directory.path(), "search", &["--path", "missing"], false);
    assert_eq!(missing["Err"]["code"], "not_found");
}

#[test]
fn search_pages_reject_changed_queries_and_invalid_limits_through_worker() {
    let directory = tempfile::tempdir().unwrap();
    for name in ["a", "b"] {
        fs::write(directory.path().join(name), "data").unwrap();
    }
    let first = run(directory.path(), "search", &["--limit", "1"], true);
    let token = first["Ok"]["result"]["version"].as_str().unwrap();
    let changed = run(
        directory.path(),
        "search",
        &["--offset", "1", "--expected-version", token, "--hidden"],
        false,
    );
    assert_eq!(changed["Err"]["code"], "changed");
    for args in [
        vec!["--offset", "1"],
        vec!["--max-depth", "0"],
        vec!["--max-depth", "33"],
        vec!["--max-entries", "10001"],
        vec!["--min-size", "4", "--max-size", "3"],
        vec!["--name", ""],
        vec!["--limit", "501"],
        vec![
            "--modified-after",
            "2026-10-02T00:00:00Z",
            "--modified-before",
            "2026-10-01T00:00:00Z",
        ],
    ] {
        assert_eq!(
            run(directory.path(), "search", &args, false)["Err"]["code"],
            "invalid_options"
        );
    }
    let escaped = run(directory.path(), "search", &["--path", "../outside"], false);
    assert_eq!(escaped["Err"]["code"], "outside_root");
}

#[test]
fn search_links_cannot_escape_root_or_repeat_directories() {
    use std::os::unix::fs::symlink;
    let directory = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("must-not-find"), "secret").unwrap();
    symlink(outside.path(), directory.path().join("outside")).unwrap();
    symlink(".", directory.path().join("loop")).unwrap();
    let result = run(
        directory.path(),
        "search",
        &["--max-depth", "32", "--follow-links"],
        true,
    );
    assert_eq!(result["Ok"]["result"]["total"], 2);
    assert_eq!(result["Ok"]["result"]["directories_scanned"], 1);
    let links = result["Ok"]["result"]["entries"].as_array().unwrap();
    assert!(links.iter().all(|entry| entry["file"]["kind"] == "symlink"));
}
