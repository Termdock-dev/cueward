use super::tests::Fixture;
use super::*;
use std::fs;
use std::os::unix::fs::symlink;

pub(super) fn search(f: &Fixture, opts: SearchOptions) -> Result<FileSearch, FileError> {
    let FileResponse::Search(result) = f.run(".", FileAction::Search(opts))? else {
        panic!("search")
    };
    Ok(result)
}

pub(super) fn names(result: &FileSearch) -> Vec<&str> {
    result
        .entries
        .iter()
        .map(|entry| entry.relative_path.as_str())
        .collect()
}

#[test]
fn search_depth_hidden_and_scope_are_explicit_without_reading_contents() {
    let f = Fixture::new();
    for dir in ["dir", "dir/nested", ".hidden", "package.app"] {
        fs::create_dir(f.0.path().join(dir)).unwrap();
    }
    for name in [
        "a",
        "dir/b",
        "dir/nested/c",
        ".hidden/secret",
        "package.app/data",
    ] {
        f.write(name, "private content, not a filename query");
    }
    let shallow = search(&f, SearchOptions::default()).unwrap();
    assert_eq!(names(&shallow), ["a", "dir", "package.app"]);
    assert_eq!(shallow.query.max_depth, 1);
    assert_eq!(shallow.directories_scanned, 1);
    assert_eq!(shallow.entries_observed, 4);
    assert_eq!(shallow.depth_boundary_directories, 2);
    assert!(shallow.enumeration_complete);
    let deep = search(
        &f,
        SearchOptions {
            max_depth: 2,
            ..SearchOptions::default()
        },
    )
    .unwrap();
    assert_eq!(
        names(&deep),
        [
            "a",
            "dir",
            "dir/b",
            "dir/nested",
            "package.app",
            "package.app/data"
        ]
    );
    assert_eq!(deep.depth_boundary_directories, 1);
    assert_eq!(deep.directories_scanned, 3);
    let hidden = search(
        &f,
        SearchOptions {
            hidden: true,
            max_depth: 3,
            kind: Some(FileKind::File),
            ..SearchOptions::default()
        },
    )
    .unwrap();
    assert_eq!(
        names(&hidden),
        [
            ".hidden/secret",
            "a",
            "dir/b",
            "dir/nested/c",
            "package.app/data"
        ]
    );
    assert_eq!(hidden.depth_boundary_directories, 0);
    assert!(
        search(
            &f,
            SearchOptions {
                name: Some("private content".into()),
                max_depth: 3,
                ..SearchOptions::default()
            }
        )
        .unwrap()
        .entries
        .is_empty()
    );
    let request = f.request(
        "dir",
        FileAction::Search(SearchOptions {
            max_depth: 3,
            kind: Some(FileKind::File),
            ..SearchOptions::default()
        }),
    );
    let FileResponse::Search(subtree) = execute_worker(&request).unwrap() else {
        panic!("search")
    };
    assert_eq!(names(&subtree), ["dir/b", "dir/nested/c"]);
    assert_eq!(subtree.directories_scanned, 2);
}

#[test]
fn literal_basename_type_size_and_fractional_dates_combine() {
    let f = Fixture::new();
    for (name, body) in [
        ("臺灣[1]\n<external>", "123"),
        ("臺灣[2]", "12345"),
        ("UPPER", "123"),
        ("other", "123"),
    ] {
        f.write(name, body);
    }
    fs::create_dir(f.0.path().join("臺灣-dir")).unwrap();
    let time = std::time::UNIX_EPOCH + std::time::Duration::from_secs(100);
    for (name, nanos) in [("臺灣[1]\n<external>", 1), ("臺灣[2]", 2)] {
        File::open(f.0.path().join(name))
            .unwrap()
            .set_times(
                fs::FileTimes::new().set_modified(time + std::time::Duration::from_nanos(nanos)),
            )
            .unwrap();
    }
    let cutoff = chrono::DateTime::parse_from_rfc3339("1970-01-01T08:01:40.000000001+08:00")
        .unwrap()
        .to_utc();
    let result = search(
        &f,
        SearchOptions {
            name: Some("臺灣[".into()),
            kind: Some(FileKind::File),
            min_size: Some(3),
            max_size: Some(3),
            modified_after: Some(cutoff),
            modified_before: Some(cutoff),
            ..SearchOptions::default()
        },
    )
    .unwrap();
    assert_eq!(names(&result), ["臺灣[1]\n<external>"]);
    for name in ["臺灣*", "臺灣[1]/", "upper"] {
        assert_eq!(
            search(
                &f,
                SearchOptions {
                    name: Some(name.into()),
                    ..SearchOptions::default()
                }
            )
            .unwrap()
            .total,
            0
        );
    }
    let newer = search(
        &f,
        SearchOptions {
            name: Some("臺灣[".into()),
            modified_after: Some(cutoff + chrono::Duration::nanoseconds(1)),
            ..SearchOptions::default()
        },
    )
    .unwrap();
    assert_eq!(names(&newer), ["臺灣[2]"]);
}

#[test]
fn symlinks_are_results_but_never_recursed_even_with_follow_links() {
    let f = Fixture::new();
    let outside = Fixture::new();
    outside.write("outside-only", "secret");
    fs::create_dir(f.0.path().join("dir")).unwrap();
    f.write("dir/a", "a");
    symlink("dir", f.0.path().join("alias")).unwrap();
    symlink(".", f.0.path().join("loop")).unwrap();
    symlink(outside.0.path(), f.0.path().join("outside")).unwrap();
    symlink("missing", f.0.path().join("broken")).unwrap();
    let mut request = f.request(
        ".",
        FileAction::Search(SearchOptions {
            max_depth: 32,
            ..SearchOptions::default()
        }),
    );
    request.follow_links = true;
    let FileResponse::Search(result) = execute_worker(&request).unwrap() else {
        panic!("search")
    };
    assert_eq!(
        names(&result),
        ["alias", "broken", "dir", "dir/a", "loop", "outside"]
    );
    assert_eq!(result.directories_scanned, 2);
    request.path = "alias".into();
    let FileResponse::Search(result) = execute_worker(&request).unwrap() else {
        panic!("search")
    };
    assert_eq!(names(&result), ["dir/a"]);
    assert!(result.entries[0].file.requested_path.ends_with("/alias/a"));
    request.path = "outside".into();
    assert_eq!(
        execute_worker(&request).unwrap_err().code,
        FileErrorCode::OutsideRoot
    );
    request.path = "alias".into();
    request.follow_links = false;
    assert_eq!(
        execute_worker(&request).unwrap_err().code,
        FileErrorCode::SymlinkDisallowed
    );
    let links = search(
        &f,
        SearchOptions {
            kind: Some(FileKind::Symlink),
            ..SearchOptions::default()
        },
    )
    .unwrap();
    assert_eq!(names(&links), ["alias", "broken", "loop", "outside"]);
}

#[test]
fn hardlinks_and_unicode_normalization_names_remain_distinct() {
    let f = Fixture::new();
    // APFS can normalize within one directory, so use separate directories.
    for dir in ["a", "b"] {
        fs::create_dir(f.0.path().join(dir)).unwrap();
    }
    f.write("a/é", "one");
    f.write("b/e\u{301}", "two");
    fs::hard_link(f.0.path().join("a/é"), f.0.path().join("hardlink")).unwrap();
    let result = search(
        &f,
        SearchOptions {
            max_depth: 2,
            kind: Some(FileKind::File),
            ..SearchOptions::default()
        },
    )
    .unwrap();
    assert_eq!(result.total, 3);
    let original = result
        .entries
        .iter()
        .find(|entry| entry.relative_path.starts_with("a/"))
        .unwrap();
    let link = result
        .entries
        .iter()
        .find(|entry| entry.relative_path == "hardlink")
        .unwrap();
    assert_eq!(original.file.identity, link.file.identity);
}
