use super::*;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::fs;
use std::os::unix::fs::symlink;

struct Fixture {
    root: tempfile::TempDir,
    locks: tempfile::TempDir,
}
impl Fixture {
    fn new() -> Self {
        Self {
            root: tempfile::tempdir().unwrap(),
            locks: tempfile::tempdir().unwrap(),
        }
    }
    fn request(&self, action: FinderAction) -> FinderRequest {
        FinderRequest {
            root: self.root.path().to_owned(),
            action,
        }
    }
    fn reveal(&self, path: &str) -> FinderRequest {
        self.request(FinderAction::Reveal {
            path: path.into(),
            follow_links: false,
            expected_version: None,
        })
    }
    fn native(&self) -> Fake {
        Fake {
            root: self.root.path().to_owned(),
            locks: self.locks.path().to_owned(),
            raw: RefCell::new(None),
            pids: RefCell::new(VecDeque::new()),
            calls: RefCell::new(Vec::new()),
            foreground: Cell::new(Some(10)),
            effect: "",
        }
    }
    fn execute(&self, request: &FinderRequest, native: &Fake) -> Result<FinderResponse, FileError> {
        execute(request, native, Some(self.locks.path()))
    }
}

struct Fake {
    root: PathBuf,
    locks: PathBuf,
    raw: RefCell<Option<RawContext>>,
    pids: RefCell<VecDeque<Option<i32>>>,
    calls: RefCell<Vec<String>>,
    foreground: Cell<Option<i32>>,
    effect: &'static str,
}
impl Native for Fake {
    fn finder_pid(&self) -> Result<Option<i32>, FileError> {
        self.calls.borrow_mut().push("pid".into());
        Ok(self.pids.borrow_mut().pop_front().unwrap_or(Some(123)))
    }
    fn foreground_pid(&self) -> Option<i32> {
        self.foreground.get()
    }
    fn context(&self, _: usize) -> Result<RawContext, FileError> {
        self.calls.borrow_mut().push("context".into());
        if self.effect == "permission" {
            return Err(FileError::new(
                FileErrorCode::PermissionDenied,
                "native -1743",
            ));
        }
        if self.effect == "root_change" {
            fs::write(self.root.join("new"), "new").unwrap();
        }
        if self.effect == "context_foreground" {
            self.foreground.set(Some(11));
        }
        Ok(self.raw.borrow_mut().take().unwrap_or(RawContext {
            window_count: 0,
            front_window: None,
            selection: vec![],
        }))
    }
    fn reveal(&self, path: &Path) -> Result<(), FileError> {
        assert!(crate::window::lock_path(&self.locks.join("finder-reveal.lock")).is_err());
        assert!(crate::window::lock_path(&self.locks.join("input-123.lock")).is_err());
        self.calls
            .borrow_mut()
            .push(format!("reveal:{}", path.display()));
        if self.effect == "file_change" {
            fs::write(path, "changed during delivery").unwrap();
        }
        self.foreground.set(Some(123));
        Ok(())
    }
}

fn raw(path: &Path) -> RawItem {
    let encoded = urlencoding::encode(path.to_str().unwrap()).replace("%2F", "/");
    RawItem::Available {
        url: format!("file://{encoded}"),
    }
}

fn context_result(response: FinderResponse) -> FinderContext {
    let FinderResponse::FinderContext(value) = response else {
        panic!("context")
    };
    value
}

#[test]
fn context_keeps_local_names_links_errors_and_outside_scope_distinct() {
    let f = Fixture::new();
    let outside = tempfile::tempdir().unwrap();
    let name = "臺灣\n<external> %2F.txt";
    fs::write(f.root.path().join(name), "original").unwrap();
    symlink(outside.path(), f.root.path().join("link")).unwrap();
    let native = f.native();
    *native.raw.borrow_mut() = Some(RawContext {
        window_count: 1,
        front_window: Some(RawWindow {
            id: 7,
            location: raw(outside.path()),
        }),
        selection: vec![
            raw(&f.root.path().join(name)),
            raw(outside.path()),
            raw(&f.root.path().join("missing")),
            raw(&f.root.path().join("link")),
            RawItem::Available {
                url: "x-apple-finder:recent".into(),
            },
            RawItem::Error {
                error: FileError::new(FileErrorCode::PermissionDenied, "native failure"),
            },
        ],
    });
    let result = context_result(
        f.execute(
            &f.request(FinderAction::Context { max_items: 100 }),
            &native,
        )
        .unwrap(),
    );
    assert!(!result.activation_requested);
    assert_eq!(result.foreground.foreground_changed, Some(false));
    assert!(matches!(
        result.front_window.unwrap().location,
        FinderItem::OutsideScope
    ));
    let selection = result.selection.unwrap();
    assert!(
        matches!(&selection[0], FinderItem::Available { relative_path, file } if relative_path == name && file.name == name)
    );
    assert!(matches!(&selection[1], FinderItem::OutsideScope));
    assert!(
        matches!(&selection[2], FinderItem::Error { error } if error.code == FileErrorCode::NotFound)
    );
    assert!(
        matches!(&selection[3], FinderItem::Available { file, .. } if file.kind == FileKind::Symlink)
    );
    assert!(
        matches!(&selection[4], FinderItem::Error { error } if error.code == FileErrorCode::UnsupportedType)
    );
    assert!(
        matches!(&selection[5], FinderItem::Error { error } if error.code == FileErrorCode::PermissionDenied)
    );
    assert!(result.selection_complete);
    assert_eq!(fs::read(f.root.path().join(name)).unwrap(), b"original");
}

#[test]
fn not_running_no_window_and_unknown_foreground_do_not_become_fake_empty_results() {
    let f = Fixture::new();
    let native = f.native();
    native.pids.borrow_mut().extend([None, None]);
    native.foreground.set(None);
    let result = context_result(
        f.execute(&f.request(FinderAction::Context { max_items: 1 }), &native)
            .unwrap(),
    );
    assert!(result.selection.is_none());
    assert!(result.window_count.is_none());
    assert!(!result.selection_complete);
    assert_eq!(result.foreground.foreground_changed, None);
    assert!(!native.calls.borrow().iter().any(|s| s == "context"));
    let mut native = f.native();
    native.effect = "context_foreground";
    let result = context_result(
        f.execute(&f.request(FinderAction::Context { max_items: 1 }), &native)
            .unwrap(),
    );
    assert_eq!(result.window_count, Some(0));
    assert!(result.front_window.is_none());
    assert!(result.selection.unwrap().is_empty());
    assert!(result.selection_complete);
    assert_eq!(result.foreground.foreground_changed, Some(true));
}

#[test]
fn context_enforces_budgets_and_propagates_permission_process_and_root_changes() {
    let f = Fixture::new();
    for budget in [0, 501] {
        let native = f.native();
        assert_eq!(
            f.execute(
                &f.request(FinderAction::Context { max_items: budget }),
                &native
            )
            .unwrap_err()
            .code,
            FileErrorCode::InvalidOptions
        );
        assert!(native.calls.borrow().is_empty());
    }
    let native = f.native();
    *native.raw.borrow_mut() = Some(RawContext {
        window_count: 0,
        front_window: None,
        selection: vec![raw(f.root.path()), raw(f.root.path())],
    });
    assert_eq!(
        f.execute(&f.request(FinderAction::Context { max_items: 1 }), &native)
            .unwrap_err()
            .code,
        FileErrorCode::ScanLimit
    );
    for (effect, code) in [
        ("permission", FileErrorCode::PermissionDenied),
        ("root_change", FileErrorCode::Changed),
    ] {
        let mut native = f.native();
        native.effect = effect;
        assert_eq!(
            f.execute(&f.request(FinderAction::Context { max_items: 10 }), &native)
                .unwrap_err()
                .code,
            code
        );
    }
    let native = f.native();
    native.pids.borrow_mut().extend([Some(123), Some(456)]);
    assert_eq!(
        f.execute(&f.request(FinderAction::Context { max_items: 10 }), &native)
            .unwrap_err()
            .code,
        FileErrorCode::Changed
    );
}

#[test]
fn reveal_uses_exact_scoped_path_shared_locks_and_conservative_delivery() {
    let f = Fixture::new();
    let name = "臺灣\n<external>.txt";
    fs::write(f.root.path().join(name), "source").unwrap();
    let native = f.native();
    let FinderResponse::FinderReveal(result) = f.execute(&f.reveal(name), &native).unwrap() else {
        panic!("reveal")
    };
    assert!(matches!(result.status, RevealStatus::SentUnverified));
    assert!(result.activation_requested && result.selection_change_requested);
    assert_eq!(result.foreground.foreground_changed, Some(true));
    assert!(result.post_check.is_ok());
    assert_eq!(
        native
            .calls
            .borrow()
            .iter()
            .filter(|s| s.starts_with("reveal:"))
            .count(),
        1
    );
    assert!(crate::window::lock_path(&f.locks.path().join("finder-reveal.lock")).is_ok());
    assert_eq!(fs::read(f.root.path().join(name)).unwrap(), b"source");
}

#[test]
fn reveal_rejects_bad_scope_stale_versions_special_files_and_links_before_submission() {
    let f = Fixture::new();
    fs::write(f.root.path().join("a"), "source").unwrap();
    symlink("a", f.root.path().join("link")).unwrap();
    symlink("/etc/passwd", f.root.path().join("outside")).unwrap();
    for (path, code) in [
        ("../escape", FileErrorCode::OutsideRoot),
        ("/etc/passwd", FileErrorCode::OutsideRoot),
        ("missing", FileErrorCode::NotFound),
        ("link", FileErrorCode::UnsupportedType),
    ] {
        let native = f.native();
        assert_eq!(f.execute(&f.reveal(path), &native).unwrap_err().code, code);
        assert!(native.calls.borrow().is_empty());
    }
    let native = f.native();
    let mut request = f.reveal("a");
    let FinderAction::Reveal {
        expected_version, ..
    } = &mut request.action
    else {
        panic!()
    };
    *expected_version = Some("stale".into());
    assert_eq!(
        f.execute(&request, &native).unwrap_err().code,
        FileErrorCode::Changed
    );
    assert!(native.calls.borrow().is_empty());
    let mut request = f.reveal("outside");
    let FinderAction::Reveal { follow_links, .. } = &mut request.action else {
        panic!()
    };
    *follow_links = true;
    assert_eq!(
        f.execute(&request, &native).unwrap_err().code,
        FileErrorCode::OutsideRoot
    );
    assert!(native.calls.borrow().is_empty());
}

#[test]
fn reveal_does_not_submit_when_global_or_recipient_lock_is_busy_or_pid_changes() {
    let f = Fixture::new();
    fs::write(f.root.path().join("a"), "source").unwrap();
    for name in ["finder-reveal.lock", "input-123.lock"] {
        let _held = crate::window::lock_path(&f.locks.path().join(name)).unwrap();
        let native = f.native();
        assert_eq!(
            f.execute(&f.reveal("a"), &native).unwrap_err().code,
            FileErrorCode::Unavailable
        );
        assert!(
            !native
                .calls
                .borrow()
                .iter()
                .any(|s| s.starts_with("reveal:"))
        );
    }
    let native = f.native();
    native.pids.borrow_mut().extend([Some(123), Some(456)]);
    assert_eq!(
        f.execute(&f.reveal("a"), &native).unwrap_err().code,
        FileErrorCode::Changed
    );
    assert!(
        !native
            .calls
            .borrow()
            .iter()
            .any(|s| s.starts_with("reveal:"))
    );
}

#[test]
fn post_submission_file_change_remains_a_sent_result_without_an_automatic_retry() {
    let f = Fixture::new();
    fs::write(f.root.path().join("a"), "source").unwrap();
    let mut native = f.native();
    native.effect = "file_change";
    let FinderResponse::FinderReveal(result) = f.execute(&f.reveal("a"), &native).unwrap() else {
        panic!()
    };
    assert!(matches!(result.status, RevealStatus::SentUnverified));
    assert_eq!(result.post_check.unwrap_err().code, FileErrorCode::Changed);
    assert_eq!(
        native
            .calls
            .borrow()
            .iter()
            .filter(|s| s.starts_with("reveal:"))
            .count(),
        1
    );
}

#[test]
fn finder_url_decoding_is_exact_and_rejects_remote_virtual_or_invalid_paths() {
    let f = Fixture::new();
    let root = observe(f.root.path(), Path::new("."), false, None).unwrap();
    let request = f.request(FinderAction::Context { max_items: 1 });
    for (url, code) in [
        ("file://server/etc/passwd", FileErrorCode::UnsupportedType),
        ("file:///invalid%XX", FileErrorCode::UnsupportedType),
        ("file:///invalid%FF", FileErrorCode::UnsupportedPathEncoding),
        ("file:///invalid%00", FileErrorCode::InvalidOptions),
        ("file:///file?query", FileErrorCode::UnsupportedType),
    ] {
        assert!(
            matches!(paths::item(&request, &root, RawItem::Available { url: url.into() }), FinderItem::Error { error } if error.code == code)
        );
    }
    let prefix_sibling = PathBuf::from(format!("{}-other/a", root.path));
    assert!(matches!(
        paths::item(&request, &root, raw(&prefix_sibling)),
        FinderItem::OutsideScope
    ));
}

#[test]
fn reveal_requires_materialized_regular_items_and_follows_only_explicit_scoped_links() {
    let f = Fixture::new();
    fs::write(f.root.path().join("a"), "source").unwrap();
    let mut file = observe(f.root.path(), Path::new("a"), false, None).unwrap();
    file.data_state = DataState::Dataless;
    assert_eq!(
        require_revealable(&file).unwrap_err().code,
        FileErrorCode::Unavailable
    );
    file.data_state = DataState::NotDataless;
    file.kind = FileKind::Other;
    assert_eq!(
        require_revealable(&file).unwrap_err().code,
        FileErrorCode::UnsupportedType
    );
    symlink("a", f.root.path().join("link")).unwrap();
    let mut request = f.reveal("link");
    let FinderAction::Reveal { follow_links, .. } = &mut request.action else {
        panic!()
    };
    *follow_links = true;
    let native = f.native();
    let FinderResponse::FinderReveal(result) = f.execute(&request, &native).unwrap() else {
        panic!()
    };
    assert_eq!(result.file.kind, FileKind::File);
    assert_eq!(
        result.file.path,
        f.root
            .path()
            .join("a")
            .canonicalize()
            .unwrap()
            .to_str()
            .unwrap()
    );
    assert_eq!(fs::read(f.root.path().join("a")).unwrap(), b"source");
}
