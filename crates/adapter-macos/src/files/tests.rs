use super::*;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};

pub(super) struct Fixture(pub tempfile::TempDir);

impl Fixture {
    pub(super) fn new() -> Self {
        Self(tempfile::tempdir().unwrap())
    }
    pub(super) fn write(&self, name: &str, bytes: impl AsRef<[u8]>) {
        fs::write(self.0.path().join(name), bytes).unwrap();
    }
    pub(super) fn request(&self, path: &str, action: FileAction) -> FileRequest {
        FileRequest {
            root: self.0.path().to_owned(),
            path: path.into(),
            follow_links: false,
            expected_version: None,
            action,
        }
    }
    pub(super) fn run(&self, path: &str, action: FileAction) -> Result<FileResponse, FileError> {
        execute_worker(&self.request(path, action))
    }
}

fn options() -> ListOptions {
    ListOptions {
        limit: 100,
        offset: 0,
        hidden: false,
        sort: FileSort::Name,
        descending: false,
    }
}

fn listing(response: FileResponse) -> FileListing {
    let FileResponse::List(value) = response else {
        panic!("list")
    };
    value
}

#[test]
fn list_orders_unicode_names_paginates_and_preserves_hidden_metadata() {
    let f = Fixture::new();
    for (name, body) in [
        ("z", "three"),
        ("a", "1"),
        ("臺灣\n<external>", "12"),
        (".hidden", ""),
    ] {
        f.write(name, body);
    }
    let mut opts = options();
    opts.limit = 2;
    let first = listing(f.run(".", FileAction::List(opts.clone())).unwrap());
    assert_eq!(
        first
            .entries
            .iter()
            .map(|e| e.name.as_str())
            .collect::<Vec<_>>(),
        ["a", "z"]
    );
    assert_eq!(first.total, 3);
    assert_eq!(first.next_offset, Some(2));
    assert!(first.enumeration_complete);
    opts.offset = 2;
    let mut request = f.request(".", FileAction::List(opts));
    request.expected_version = Some(first.version.clone());
    let second = listing(execute_worker(&request).unwrap());
    assert_eq!(second.entries[0].name, "臺灣\n<external>");
    assert_eq!(second.next_offset, None);
    f.write(".hidden", "changed");
    assert_eq!(
        execute_worker(&request).unwrap_err().code,
        FileErrorCode::Changed
    );
    let mut opts = options();
    opts.hidden = true;
    opts.sort = FileSort::Size;
    opts.descending = true;
    let all = listing(f.run(".", FileAction::List(opts)).unwrap());
    assert_eq!(
        all.entries
            .iter()
            .map(|e| e.name.as_str())
            .collect::<Vec<_>>(),
        [".hidden", "z", "臺灣\n<external>", "a"]
    );
}

#[test]
fn list_token_detects_child_changes_even_when_directory_metadata_does_not_change() {
    let f = Fixture::new();
    f.write("a", "a");
    f.write("b", "bb");
    let first = listing(f.run(".", FileAction::List(options())).unwrap());
    let before = MacFiles.stamp(&fs::metadata(f.0.path()).unwrap());
    f.write("a", "longer");
    assert_eq!(
        before.version,
        MacFiles.stamp(&fs::metadata(f.0.path()).unwrap()).version
    );
    let mut request = f.request(".", FileAction::List(options()));
    request.expected_version = Some(first.version);
    assert_eq!(
        execute_worker(&request).unwrap_err().code,
        FileErrorCode::Changed
    );
}

#[test]
fn modified_sort_uses_time_values_including_fractional_seconds() {
    let f = Fixture::new();
    f.write("a-later", "a");
    f.write("z-earlier", "z");
    let base = std::time::UNIX_EPOCH + std::time::Duration::from_secs(100);
    for (name, time) in [
        ("a-later", base + std::time::Duration::from_nanos(1)),
        ("z-earlier", base),
    ] {
        File::open(f.0.path().join(name))
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(time))
            .unwrap();
    }
    let mut opts = options();
    opts.sort = FileSort::Modified;
    let result = listing(f.run(".", FileAction::List(opts)).unwrap());
    assert_eq!(
        result
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        ["z-earlier", "a-later"]
    );
}

#[test]
fn rejects_unscoped_paths_bad_pages_and_wrong_types() {
    let f = Fixture::new();
    f.write("a", "a");
    for path in ["../a", "/etc/passwd", "nested/../a"] {
        assert_eq!(
            f.run(path, FileAction::Info).unwrap_err().code,
            FileErrorCode::OutsideRoot
        );
    }
    let mut request = f.request("a", FileAction::Info);
    request.root = "relative".into();
    assert_eq!(
        execute_worker(&request).unwrap_err().code,
        FileErrorCode::InvalidOptions
    );
    assert_eq!(
        f.run("missing", FileAction::Info).unwrap_err().code,
        FileErrorCode::NotFound
    );
    assert_eq!(
        f.run("a", FileAction::List(options())).unwrap_err().code,
        FileErrorCode::UnsupportedType
    );
    for (limit, offset) in [(0, 0), (501, 0), (1, 1)] {
        let mut opts = options();
        opts.limit = limit;
        opts.offset = offset;
        assert_eq!(
            f.run(".", FileAction::List(opts)).unwrap_err().code,
            FileErrorCode::InvalidOptions
        );
    }
}

#[test]
fn info_reports_identity_permissions_and_leaf_link_without_following() {
    let f = Fixture::new();
    f.write("a", "123");
    fs::set_permissions(f.0.path().join("a"), fs::Permissions::from_mode(0o400)).unwrap();
    let FileResponse::Info(info) = f.run("a", FileAction::Info).unwrap() else {
        panic!("info")
    };
    assert_eq!(info.kind, FileKind::File);
    assert_eq!(info.size, 3);
    assert!(info.readonly);
    assert!(info.modified.is_some());
    assert!(info.created.is_some());
    assert!(info.mode.is_some());
    assert!(!info.identity.is_empty());
    assert!(!info.version.is_empty());
    symlink("absent", f.0.path().join("broken")).unwrap();
    let FileResponse::Info(info) = f.run("broken", FileAction::Info).unwrap() else {
        panic!("info")
    };
    assert_eq!(info.kind, FileKind::Symlink);
    assert_eq!(info.link_target.as_deref(), Some("absent"));
    assert!(info.resolved_path.is_none());
}

#[test]
fn symlink_following_is_explicit_and_stays_within_root() {
    let f = Fixture::new();
    f.write("a", "a");
    symlink("a", f.0.path().join("link")).unwrap();
    symlink("/etc", f.0.path().join("outside")).unwrap();
    fs::create_dir(f.0.path().join("dir")).unwrap();
    f.write("dir/b", "b");
    symlink("dir", f.0.path().join("parent")).unwrap();
    assert_eq!(
        f.run("parent/b", FileAction::Info).unwrap_err().code,
        FileErrorCode::SymlinkDisallowed
    );
    let mut request = f.request("link", FileAction::Info);
    request.follow_links = true;
    let FileResponse::Info(info) = execute_worker(&request).unwrap() else {
        panic!("info")
    };
    assert_eq!(info.kind, FileKind::File);
    assert!(info.path.ends_with("/a"));
    assert!(info.requested_path.ends_with("/link"));
    request.path = "outside/passwd".into();
    assert_eq!(
        execute_worker(&request).unwrap_err().code,
        FileErrorCode::OutsideRoot
    );
    // The open hook independently rejects a symlink in any component, including a raced parent.
    assert!(MacFiles.open_regular(&f.0.path().join("parent/b")).is_err());
}

#[test]
fn non_utf8_request_path_is_an_explicit_error_not_lossy_output() {
    use std::os::unix::ffi::OsStringExt;
    let f = Fixture::new();
    let name = std::ffi::OsString::from_vec(vec![0xff]);
    let mut request = f.request(".", FileAction::Info);
    request.path = name.into();
    assert_eq!(
        execute_worker(&request).unwrap_err().code,
        FileErrorCode::UnsupportedPathEncoding
    );
}

#[test]
fn directory_scan_cap_includes_hidden_entries_and_never_returns_partial_sort() {
    let f = Fixture::new();
    for n in 0..=MAX_DIRECTORY_ENTRIES {
        f.write(&format!(".hidden{n:05}"), "");
    }
    assert_eq!(
        f.run(".", FileAction::List(options())).unwrap_err().code,
        FileErrorCode::ScanLimit
    );
}

#[test]
fn dataless_flag_is_distinct_from_zero_size_and_info_can_report_it() {
    assert_eq!(data_state(0), DataState::NotDataless);
    assert_eq!(data_state(0x40000000), DataState::Dataless);
    struct Dataless;
    impl FilePlatform for Dataless {
        fn stamp(&self, metadata: &Metadata) -> FileStamp {
            let mut stamp = MacFiles.stamp(metadata);
            if metadata.is_file() {
                stamp.data_state = DataState::Dataless;
            }
            stamp
        }
        fn open_regular(&self, _: &Path) -> io::Result<File> {
            panic!("must not open dataless file")
        }
    }
    let f = Fixture::new();
    f.write("placeholder", "");
    let request = f.request("placeholder", FileAction::Info);
    let FileResponse::Info(info) = cueward_core::files::execute(&Dataless, &request).unwrap()
    else {
        panic!("info")
    };
    assert_eq!(info.data_state, DataState::Dataless);
    assert_eq!(info.size, 0);
    let request = f.request(
        "placeholder",
        FileAction::Read(super::reading_tests::options()),
    );
    assert_eq!(
        cueward_core::files::execute(&Dataless, &request)
            .unwrap_err()
            .code,
        FileErrorCode::Unavailable
    );
}
