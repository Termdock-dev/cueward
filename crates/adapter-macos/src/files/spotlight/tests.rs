use super::*;
use std::cell::RefCell;
use std::fs;
use std::os::unix::fs::symlink;

#[derive(Default)]
struct Fixture {
    paths: Vec<PathBuf>,
    calls: RefCell<Vec<(PathBuf, String, usize)>>,
    error: Option<FileErrorCode>,
    mutate_root: bool,
}
impl Native for Fixture {
    fn search(&self, root: &Path, query: &str, max: usize) -> Result<Vec<PathBuf>, FileError> {
        self.calls
            .borrow_mut()
            .push((root.to_owned(), query.to_owned(), max));
        if self.mutate_root {
            fs::write(root.join("new-file"), "changed").unwrap();
        }
        if let Some(code) = self.error {
            return Err(FileError::new(code, "fixture"));
        }
        Ok(self.paths.clone())
    }
}

fn request(root: &Path) -> SpotlightRequest {
    SpotlightRequest {
        root: root.to_owned(),
        text: "臺灣 report".into(),
        max_depth: 1,
        hidden: false,
        max_candidates: 100,
        limit: 100,
    }
}
fn result(request: &SpotlightRequest, native: &impl Native) -> SpotlightResult {
    let SpotlightResponse::Spotlight(result) = execute(request, native).unwrap();
    result
}

#[test]
fn zero_index_candidates_never_claim_complete_filesystem_or_content_coverage() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("unindexed.txt"), "臺灣 report").unwrap();
    let fixture = Fixture::default();
    let req = request(dir.path());
    let output = result(&req, &fixture);
    assert!(output.entries.is_empty());
    assert!(output.query_completed);
    assert!(!output.enumeration_complete);
    assert!(!output.content_verified);
    assert!(matches!(output.index_coverage, IndexCoverage::Unknown));
    assert!(matches!(output.source, SpotlightSource::Spotlight));
    assert_eq!(
        fixture.calls.borrow()[0].0,
        dir.path().canonicalize().unwrap()
    );
    assert_eq!(fixture.calls.borrow()[0].2, 100);
    assert_eq!(
        fs::read_to_string(dir.path().join("unindexed.txt")).unwrap(),
        "臺灣 report"
    );
}

#[test]
fn sorted_bounded_results_preserve_unicode_newlines_and_metadata_without_content() {
    let dir = tempfile::tempdir().unwrap();
    let names = [
        "臺灣\n.txt",
        "a space.txt",
        "z.txt",
        "e\u{301}.txt",
        "é.txt",
    ];
    for name in names {
        fs::write(
            dir.path().join(name),
            "external </external> not matching query",
        )
        .unwrap();
    }
    let native = Fixture {
        paths: names
            .iter()
            .map(|name| dir.path().canonicalize().unwrap().join(name))
            .collect(),
        ..Default::default()
    };
    let mut req = request(dir.path());
    req.limit = 2;
    let output = result(&req, &native);
    assert_eq!(output.candidate_count, 5);
    assert_eq!(output.eligible_count, 5);
    assert_eq!(output.available_count, 5);
    assert_eq!(output.error_count, 0);
    assert!(output.truncated);
    assert_eq!(
        output
            .entries
            .iter()
            .map(SpotlightEntry::relative_path)
            .collect::<Vec<_>>(),
        ["a space.txt", "e\u{301}.txt"]
    );
    let json = serde_json::to_string(&output).unwrap();
    assert!(!json.contains("not matching query"));
    req.limit = 10;
    assert_eq!(result(&req, &native).entries.len(), 5);
}

#[test]
fn filters_exclude_outside_hidden_depth_duplicate_and_non_regular_candidates() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    fs::create_dir(root.join("sub")).unwrap();
    fs::write(root.join("sub/deep"), "data").unwrap();
    fs::write(root.join(".hidden"), "data").unwrap();
    fs::write(root.join("visible"), "data").unwrap();
    let outside = tempfile::tempdir().unwrap();
    symlink(outside.path().join("missing"), root.join("leaf-link")).unwrap();
    let paths = [
        root.join("visible"),
        root.join("visible"),
        root.join(".hidden"),
        root.join("sub/deep"),
        root.join("sub"),
        root.join("leaf-link"),
        root.clone(),
        root.join("../outside"),
        outside.path().join("missing"),
        PathBuf::from(format!("{}-sibling/missing", root.display())),
    ];
    let native = Fixture {
        paths: paths.into(),
        ..Default::default()
    };
    let mut req = request(&root);
    let output = result(&req, &native);
    assert_eq!(output.entries.len(), 1);
    assert_eq!(output.excluded.outside_scope, 4);
    assert_eq!(output.excluded.hidden, 1);
    assert_eq!(output.excluded.depth, 1);
    assert_eq!(output.excluded.non_regular, 2);
    assert_eq!(output.excluded.duplicate, 1);
    req.hidden = true;
    req.max_depth = 2;
    let output = result(&req, &native);
    assert_eq!(output.entries.len(), 3);
    assert_eq!(output.excluded.hidden, 0);
    assert_eq!(output.excluded.depth, 0);
}

#[test]
fn stale_paths_and_ancestor_symlinks_remain_errors_instead_of_silent_zero_matches() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("private.txt"), "secret").unwrap();
    symlink(outside.path(), root.join("link")).unwrap();
    let native = Fixture {
        paths: vec![root.join("gone"), root.join("link/private.txt")],
        ..Default::default()
    };
    let mut req = request(&root);
    req.max_depth = 2;
    let output = result(&req, &native);
    assert_eq!(output.available_count, 0);
    assert_eq!(output.error_count, 2);
    assert_eq!(output.eligible_count, 2);
    assert!(
        matches!(&output.entries[0], SpotlightEntry::Error { error, .. } if error.code == FileErrorCode::NotFound)
    );
    assert!(
        matches!(&output.entries[1], SpotlightEntry::Error { error, .. } if error.code == FileErrorCode::SymlinkDisallowed)
    );
    assert!(!serde_json::to_string(&output).unwrap().contains("secret"));
}

#[test]
fn candidate_budget_applies_before_filters_and_never_returns_partial_success() {
    let dir = tempfile::tempdir().unwrap();
    let native = Fixture {
        paths: vec![PathBuf::from("/outside"), PathBuf::from("/another")],
        ..Default::default()
    };
    let mut req = request(dir.path());
    req.max_candidates = 1;
    req.limit = 1;
    assert_eq!(
        execute(&req, &native).unwrap_err().code,
        FileErrorCode::ScanLimit
    );
}

#[test]
fn root_changes_and_native_failures_discard_results() {
    let dir = tempfile::tempdir().unwrap();
    let req = request(dir.path());
    let native = Fixture {
        mutate_root: true,
        ..Default::default()
    };
    assert_eq!(
        execute(&req, &native).unwrap_err().code,
        FileErrorCode::Changed
    );
    for code in [
        FileErrorCode::PermissionDenied,
        FileErrorCode::Unavailable,
        FileErrorCode::Timeout,
    ] {
        let native = Fixture {
            error: Some(code),
            ..Default::default()
        };
        assert_eq!(execute(&req, &native).unwrap_err().code, code);
    }
}

#[test]
fn explicit_directory_root_is_required_before_native_query() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("file");
    fs::write(&file, "data").unwrap();
    let native = Fixture::default();
    for (root, code) in [
        (Path::new("relative"), FileErrorCode::InvalidOptions),
        (file.as_path(), FileErrorCode::UnsupportedType),
    ] {
        assert_eq!(execute(&request(root), &native).unwrap_err().code, code);
    }
    assert!(native.calls.borrow().is_empty());
}

#[test]
fn worker_requests_validate_all_budgets_and_reject_ambiguous_text() {
    let mut req = request(Path::new("/tmp"));
    for text in ["", " ", "wild*", "wild?", "new\nline", "nul\0", "tab\t"] {
        req.text = text.into();
        assert_eq!(
            expression(&req).unwrap_err().code,
            FileErrorCode::InvalidOptions
        );
    }
    req.text = "x".repeat(1025);
    assert!(expression(&req).is_err());
    req.text = "valid".into();
    for (depth, candidates, limit) in [
        (0, 1, 1),
        (33, 1, 1),
        (1, 0, 1),
        (1, 10001, 1),
        (1, 1, 0),
        (1, 1, 501),
    ] {
        req.max_depth = depth;
        req.max_candidates = candidates;
        req.limit = limit;
        assert_eq!(
            expression(&req).unwrap_err().code,
            FileErrorCode::InvalidOptions
        );
    }
}

#[test]
fn quotes_and_backslashes_cannot_escape_the_single_content_value() {
    let mut req = request(Path::new("/tmp"));
    req.text = "a\" || kMDItemFSName == \"b\\c".into();
    assert_eq!(
        expression(&req).unwrap(),
        "kMDItemTextContent == \"*a\\\" || kMDItemFSName == \\\"b\\\\c*\"c"
    );
}

#[test]
fn a_blocking_spotlight_worker_is_stopped_without_partial_success() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let exe = dir.path().join("worker");
    fs::write(&exe, "#!/bin/sh\nexec /bin/sleep 30\n").unwrap();
    fs::set_permissions(&exe, fs::Permissions::from_mode(0o700)).unwrap();
    let start = std::time::Instant::now();
    assert_eq!(
        run(&exe, &request(dir.path()), 100).unwrap_err().code,
        FileErrorCode::Timeout
    );
    assert!(start.elapsed() < std::time::Duration::from_secs(5));
}

#[test]
fn path_budgets_and_encoding_apply_before_scope_filters_or_deduplication() {
    use std::os::unix::ffi::OsStringExt;
    let dir = tempfile::tempdir().unwrap();
    let req = request(dir.path());
    for (path, code) in [
        (
            PathBuf::from(std::ffi::OsString::from_vec(vec![b'/', 0xff])),
            FileErrorCode::UnsupportedPathEncoding,
        ),
        (
            PathBuf::from("/nul\0path"),
            FileErrorCode::UnsupportedPathEncoding,
        ),
        (
            PathBuf::from(format!("/{}", "x".repeat(16384))),
            FileErrorCode::ScanLimit,
        ),
    ] {
        let native = Fixture {
            paths: vec![path],
            ..Default::default()
        };
        assert_eq!(execute(&req, &native).unwrap_err().code, code);
    }
    let mut req = req;
    req.max_candidates = 1000;
    let native = Fixture {
        paths: vec![PathBuf::from(format!("/{}", "x".repeat(16383))); 257],
        ..Default::default()
    };
    assert_eq!(
        execute(&req, &native).unwrap_err().code,
        FileErrorCode::ScanLimit
    );
}
