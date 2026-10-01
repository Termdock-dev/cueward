use super::search_tests::{names, search};
use super::tests::Fixture;
use super::*;
use std::fs;

#[test]
fn pages_bind_query_and_unmatched_descendants_but_allow_limit_changes() {
    let f = Fixture::new();
    fs::create_dir(f.0.path().join("dir")).unwrap();
    for (name, body) in [
        ("a-match", "1"),
        ("b-match", "22"),
        ("c-match", "333"),
        ("dir/no", "4"),
        (".hidden", "5"),
    ] {
        f.write(name, body);
    }
    let options = SearchOptions {
        name: Some("match".into()),
        max_depth: 2,
        limit: 1,
        ..SearchOptions::default()
    };
    let first = search(&f, options.clone()).unwrap();
    assert_eq!(names(&first), ["a-match"]);
    assert_eq!((first.total, first.next_offset), (3, Some(1)));
    assert_eq!(search(&f, options.clone()).unwrap().version, first.version);
    let mut request = f.request(
        ".",
        FileAction::Search(SearchOptions {
            offset: 1,
            limit: 2,
            ..options.clone()
        }),
    );
    request.expected_version = Some(first.version.clone());
    let FileResponse::Search(next) = execute_worker(&request).unwrap() else {
        panic!("search")
    };
    assert_eq!(names(&next), ["b-match", "c-match"]);
    assert_eq!(next.version, first.version);
    assert_eq!(next.next_offset, None);
    for changed in [
        SearchOptions {
            name: Some("a-match".into()),
            ..options.clone()
        },
        SearchOptions {
            hidden: true,
            ..options.clone()
        },
        SearchOptions {
            max_depth: 1,
            ..options.clone()
        },
        SearchOptions {
            max_entries: 100,
            ..options.clone()
        },
        SearchOptions {
            kind: Some(FileKind::File),
            ..options.clone()
        },
        SearchOptions {
            min_size: Some(1),
            ..options.clone()
        },
    ] {
        request.action = FileAction::Search(changed);
        assert_eq!(
            execute_worker(&request).unwrap_err().code,
            FileErrorCode::Changed
        );
    }
    request.action = FileAction::Search(options.clone());
    let root_stamp = MacFiles.stamp(&fs::metadata(f.0.path()).unwrap());
    f.write("dir/no", "changed nonmatching child");
    assert_eq!(
        root_stamp.version,
        MacFiles.stamp(&fs::metadata(f.0.path()).unwrap()).version
    );
    assert_eq!(
        execute_worker(&request).unwrap_err().code,
        FileErrorCode::Changed
    );
    request.expected_version = Some(search(&f, options).unwrap().version);
    f.write(".hidden", "changed hidden");
    assert_eq!(
        execute_worker(&request).unwrap_err().code,
        FileErrorCode::Changed
    );
}

#[test]
fn scan_cap_is_global_includes_hidden_and_never_returns_incomplete_zero_matches() {
    let f = Fixture::new();
    fs::create_dir(f.0.path().join("dir")).unwrap();
    f.write(".hidden", "");
    f.write("dir/a", "");
    f.write("dir/b", "");
    let opts = SearchOptions {
        max_depth: 2,
        max_entries: 4,
        name: Some("no match".into()),
        ..SearchOptions::default()
    };
    let result = search(&f, opts.clone()).unwrap();
    assert_eq!(result.total, 0);
    assert_eq!(result.entries_observed, 4);
    assert!(result.enumeration_complete);
    assert_eq!(
        search(
            &f,
            SearchOptions {
                max_entries: 3,
                ..opts
            }
        )
        .unwrap_err()
        .code,
        FileErrorCode::ScanLimit
    );
}

#[test]
fn invalid_options_wrong_type_and_non_utf8_paths_fail_explicitly() {
    let f = Fixture::new();
    let now = chrono::Utc::now();
    for opts in [
        SearchOptions {
            max_depth: 0,
            ..SearchOptions::default()
        },
        SearchOptions {
            max_depth: 33,
            ..SearchOptions::default()
        },
        SearchOptions {
            max_entries: 0,
            ..SearchOptions::default()
        },
        SearchOptions {
            max_entries: 10001,
            ..SearchOptions::default()
        },
        SearchOptions {
            limit: 0,
            ..SearchOptions::default()
        },
        SearchOptions {
            limit: 501,
            ..SearchOptions::default()
        },
        SearchOptions {
            offset: 1,
            ..SearchOptions::default()
        },
        SearchOptions {
            name: Some("".into()),
            ..SearchOptions::default()
        },
        SearchOptions {
            min_size: Some(2),
            max_size: Some(1),
            ..SearchOptions::default()
        },
        SearchOptions {
            modified_after: Some(now),
            modified_before: Some(now - chrono::Duration::seconds(1)),
            ..SearchOptions::default()
        },
    ] {
        assert_eq!(
            search(&f, opts).unwrap_err().code,
            FileErrorCode::InvalidOptions
        );
    }
    f.write("file", "");
    assert_eq!(
        f.run("file", FileAction::Search(SearchOptions::default()))
            .unwrap_err()
            .code,
        FileErrorCode::UnsupportedType
    );
    use std::os::unix::ffi::OsStringExt;
    let mut request = f.request(".", FileAction::Search(SearchOptions::default()));
    request.path = std::ffi::OsString::from_vec(vec![0xff]).into();
    assert_eq!(
        execute_worker(&request).unwrap_err().code,
        FileErrorCode::UnsupportedPathEncoding
    );
}

#[test]
fn dataless_directory_and_permissions_are_errors_when_traversal_is_required() {
    struct DatalessDir {
        inode: u64,
    }
    impl FilePlatform for DatalessDir {
        fn stamp(&self, metadata: &Metadata) -> FileStamp {
            let mut stamp = MacFiles.stamp(metadata);
            if metadata.ino() == self.inode {
                stamp.data_state = DataState::Dataless;
            }
            stamp
        }
        fn open_regular(&self, _: &Path) -> io::Result<File> {
            panic!("search must not read content")
        }
    }
    let f = Fixture::new();
    fs::create_dir(f.0.path().join("dir")).unwrap();
    f.write("dir/a", "");
    let platform = DatalessDir {
        inode: fs::metadata(f.0.path().join("dir")).unwrap().ino(),
    };
    let request = f.request(".", FileAction::Search(SearchOptions::default()));
    let FileResponse::Search(result) = cueward_core::files::execute(&platform, &request).unwrap()
    else {
        panic!("search")
    };
    assert_eq!(result.entries[0].file.data_state, DataState::Dataless);
    let request = f.request(
        ".",
        FileAction::Search(SearchOptions {
            max_depth: 2,
            ..SearchOptions::default()
        }),
    );
    assert_eq!(
        cueward_core::files::execute(&platform, &request)
            .unwrap_err()
            .code,
        FileErrorCode::Unavailable
    );
    // This test runs as the regular macOS user. Restore permissions before cleanup.
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(f.0.path().join("dir"), fs::Permissions::from_mode(0o000)).unwrap();
    let result = execute_worker(&request);
    fs::set_permissions(f.0.path().join("dir"), fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(result.unwrap_err().code, FileErrorCode::PermissionDenied);
}

#[test]
fn concurrent_child_change_discards_all_results() {
    struct Mutate {
        path: std::path::PathBuf,
        inode: u64,
        calls: std::cell::Cell<usize>,
    }
    impl FilePlatform for Mutate {
        fn stamp(&self, metadata: &Metadata) -> FileStamp {
            let stamp = MacFiles.stamp(metadata);
            if metadata.ino() == self.inode {
                let count = self.calls.get() + 1;
                self.calls.set(count);
                if count == 2 {
                    fs::write(&self.path, "changed while scanning").unwrap();
                }
            }
            stamp
        }
        fn open_regular(&self, _: &Path) -> io::Result<File> {
            panic!("search must not read content")
        }
    }
    let f = Fixture::new();
    f.write("a", "initial");
    let path = f.0.path().join("a");
    let platform = Mutate {
        inode: fs::metadata(&path).unwrap().ino(),
        path,
        calls: std::cell::Cell::new(0),
    };
    assert_eq!(
        cueward_core::files::execute(
            &platform,
            &f.request(".", FileAction::Search(SearchOptions::default()))
        )
        .unwrap_err()
        .code,
        FileErrorCode::Changed
    );
}

#[test]
fn end_page_is_empty_and_offsets_past_total_are_rejected() {
    let f = Fixture::new();
    f.write("a", "a");
    let first = search(&f, SearchOptions::default()).unwrap();
    let mut request = f.request(
        ".",
        FileAction::Search(SearchOptions {
            offset: 1,
            ..SearchOptions::default()
        }),
    );
    request.expected_version = Some(first.version);
    let FileResponse::Search(result) = execute_worker(&request).unwrap() else {
        panic!("search")
    };
    assert_eq!(result.total, 1);
    assert!(result.entries.is_empty());
    assert_eq!(result.next_offset, None);
    request.action = FileAction::Search(SearchOptions {
        offset: usize::MAX,
        ..SearchOptions::default()
    });
    assert_eq!(
        execute_worker(&request).unwrap_err().code,
        FileErrorCode::InvalidOptions
    );
}

#[test]
fn pending_directory_replaced_by_outside_symlink_is_not_traversed() {
    struct ReplaceDir {
        directory: std::path::PathBuf,
        outside: std::path::PathBuf,
        inode: u64,
        calls: std::cell::Cell<usize>,
    }
    impl FilePlatform for ReplaceDir {
        fn stamp(&self, metadata: &Metadata) -> FileStamp {
            let stamp = MacFiles.stamp(metadata);
            if metadata.ino() == self.inode {
                let count = self.calls.get() + 1;
                self.calls.set(count);
                if count == 2 {
                    fs::rename(&self.directory, self.directory.with_extension("old")).unwrap();
                    std::os::unix::fs::symlink(&self.outside, &self.directory).unwrap();
                }
            }
            stamp
        }
        fn open_regular(&self, _: &Path) -> io::Result<File> {
            panic!("search must not read content")
        }
    }
    let f = Fixture::new();
    let outside = Fixture::new();
    outside.write("must-not-find", "secret");
    let directory = f.0.path().join("dir");
    fs::create_dir(&directory).unwrap();
    let platform = ReplaceDir {
        inode: fs::metadata(&directory).unwrap().ino(),
        directory,
        outside: outside.0.path().to_owned(),
        calls: std::cell::Cell::new(0),
    };
    let request = f.request(
        ".",
        FileAction::Search(SearchOptions {
            max_depth: 2,
            ..SearchOptions::default()
        }),
    );
    assert_eq!(
        cueward_core::files::execute(&platform, &request)
            .unwrap_err()
            .code,
        FileErrorCode::SymlinkDisallowed
    );
}
