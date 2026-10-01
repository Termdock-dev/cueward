use super::*;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
#[path = "failure_tests.rs"]
mod failures;

fn fixture() -> (tempfile::TempDir, MutationRequest) {
    let root = tempfile::tempdir().unwrap();
    fs::write(
        root.path().join("source.txt"),
        b"owned bytes\0</external>\n",
    )
    .unwrap();
    let source = super::super::observe(root.path(), Path::new("source.txt"), false, None).unwrap();
    let parent = super::super::observe(root.path(), Path::new("."), false, None).unwrap();
    let request = MutationRequest {
        root: root.path().to_owned(),
        destination: "copy.txt".into(),
        expected_parent_version: parent.version,
        action: MutationAction::Copy {
            path: "source.txt".into(),
            expected_version: source.version,
            max_bytes: 1024,
        },
    };
    (root, request)
}
fn fresh(request: &mut MutationRequest) {
    request.expected_parent_version = super::super::observe(
        &request.root,
        request
            .destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
        false,
        None,
    )
    .unwrap()
    .version;
    if let MutationAction::Copy {
        path,
        expected_version,
        ..
    } = &mut request.action
    {
        *expected_version = super::super::observe(&request.root, path, false, None)
            .unwrap()
            .version;
    }
}
fn run_request(request: &MutationRequest) -> MutationReceipt {
    let prepared = journal::create(request).unwrap();
    let receipt = execute_worker(&MutationWorkerRequest {
        operation_id: prepared.operation_id.clone(),
    })
    .unwrap();
    assert_eq!(
        serde_json::to_value(journal::load(&prepared.operation_id).unwrap()).unwrap(),
        serde_json::to_value(&receipt).unwrap()
    );
    fs::remove_dir_all(journal::directory(&prepared.operation_id).unwrap()).unwrap();
    receipt
}
fn assert_not_started(request: &MutationRequest, code: FileErrorCode) {
    let receipt = run_request(request);
    assert_eq!(receipt.status, MutationStatus::NotStarted);
    assert!(!receipt.completion_verified);
    assert_eq!(receipt.error.unwrap().code, code);
}

#[test]
fn mkdir_creates_one_empty_private_directory_and_does_not_replace() {
    let (root, mut request) = fixture();
    request.action = MutationAction::Mkdir;
    request.destination = "新資料夾\n<external>".into();
    let receipt = run_request(&request);
    assert_eq!(receipt.status, MutationStatus::Completed);
    assert!(receipt.completion_verified && receipt.destination_created == Some(true));
    let path = root.path().join(&request.destination);
    assert!(path.is_dir());
    assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o700);
    assert!(fs::read_dir(&path).unwrap().next().is_none());
    fresh(&mut request);
    assert_not_started(&request, FileErrorCode::Conflict);
    assert!(path.is_dir());
    request.destination = "missing/child".into();
    assert_not_started(&request, FileErrorCode::NotFound);
    assert!(!root.path().join("missing").exists());
}

#[test]
fn copy_verifies_content_identity_permissions_mtime_and_real_extended_attributes() {
    let (root, mut request) = fixture();
    let source = root.path().join("source.txt");
    fs::set_permissions(&source, fs::Permissions::from_mode(0o640)).unwrap();
    let file = fs::File::open(&source).unwrap();
    file.set_times(
        fs::FileTimes::new()
            .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1500000000)),
    )
    .unwrap();
    set_attribute(&source, b"com.cueward.fixture", b"tagged\0fixture");
    fs::hard_link(&source, root.path().join("hardlink.txt")).unwrap();
    fresh(&mut request);
    let original = fs::read(&source).unwrap();
    let before = fs::metadata(&source).unwrap();
    let receipt = run_request(&request);
    assert_eq!(
        receipt.status,
        MutationStatus::Completed,
        "{:?}",
        receipt.error
    );
    let destination = root.path().join("copy.txt");
    assert_eq!(fs::read(&destination).unwrap(), original);
    assert_eq!(fs::read(&source).unwrap(), original);
    assert_eq!(
        fs::read(root.path().join("hardlink.txt")).unwrap(),
        original
    );
    let after = fs::metadata(&destination).unwrap();
    assert_eq!(after.modified().unwrap(), before.modified().unwrap());
    assert_eq!(after.mode() & 0o777, 0o640);
    assert_ne!(after.ino(), before.ino());
    assert_eq!(
        receipt.source_before.as_ref().unwrap().identity,
        receipt.source_after.as_ref().unwrap().identity
    );
    let verification = receipt.verification.unwrap();
    assert!(verification.permissions_equal && verification.modified_equal);
    assert_eq!(verification.bytes, original.len() as u64);
    assert_eq!(
        attributes::digest(&file).unwrap(),
        attributes::digest(&fs::File::open(&destination).unwrap()).unwrap()
    );
}

#[test]
fn copy_conflicts_including_broken_symlinks_preserve_every_existing_byte() {
    let (root, mut request) = fixture();
    fs::write(root.path().join("copy.txt"), "existing bytes").unwrap();
    fresh(&mut request);
    assert_not_started(&request, FileErrorCode::Conflict);
    assert_eq!(
        fs::read(root.path().join("copy.txt")).unwrap(),
        b"existing bytes"
    );
    symlink("/outside/missing", root.path().join("broken")).unwrap();
    request.destination = "broken".into();
    fresh(&mut request);
    assert_not_started(&request, FileErrorCode::Conflict);
    assert_eq!(
        fs::read_link(root.path().join("broken")).unwrap(),
        PathBuf::from("/outside/missing")
    );
}

#[test]
fn byte_revision_scope_and_parent_guards_fail_before_creation() {
    let (root, mut request) = fixture();
    if let MutationAction::Copy { max_bytes, .. } = &mut request.action {
        *max_bytes = 1;
    }
    assert_not_started(&request, FileErrorCode::ScanLimit);
    if let MutationAction::Copy {
        max_bytes,
        expected_version,
        ..
    } = &mut request.action
    {
        *max_bytes = 1024;
        *expected_version = "stale".into();
    }
    assert_not_started(&request, FileErrorCode::Changed);
    fresh(&mut request);
    request.expected_parent_version = "stale".into();
    assert_not_started(&request, FileErrorCode::Changed);
    fresh(&mut request);
    for path in ["../escape", "/absolute", ".", ""] {
        request.destination = path.into();
        assert_not_started(&request, FileErrorCode::InvalidOptions);
    }
    assert!(!root.path().join("copy.txt").exists());
}

#[test]
fn directories_packages_and_symlinks_never_become_copy_sources() {
    let (root, mut request) = fixture();
    fs::create_dir(root.path().join("folder.app")).unwrap();
    symlink("source.txt", root.path().join("link")).unwrap();
    for path in ["folder.app", "link"] {
        if let MutationAction::Copy { path: p, .. } = &mut request.action {
            *p = path.into();
        }
        fresh(&mut request);
        assert_not_started(&request, FileErrorCode::UnsupportedType);
    }
    let outside = tempfile::tempdir().unwrap();
    symlink(outside.path(), root.path().join("parent-link")).unwrap();
    request.destination = "parent-link/new".into();
    assert_not_started(&request, FileErrorCode::SymlinkDisallowed);
    assert!(!outside.path().join("new").exists());
}

#[test]
fn special_permission_bits_and_large_metadata_are_rejected_before_writing() {
    let (root, mut request) = fixture();
    let path = root.path().join("source.txt");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o4644)).unwrap();
    fresh(&mut request);
    assert_not_started(&request, FileErrorCode::UnsupportedType);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    set_attribute(&path, b"com.cueward.large", &vec![42; 4 * 1024 * 1024 + 1]);
    fresh(&mut request);
    assert_not_started(&request, FileErrorCode::ScanLimit);
    assert!(!root.path().join("copy.txt").exists());
}

fn set_attribute(path: &Path, name: &[u8], value: &[u8]) {
    use std::ffi::{CString, c_char, c_void};
    use std::os::unix::ffi::OsStrExt;
    unsafe extern "C" {
        fn setxattr(
            path: *const c_char,
            name: *const c_char,
            value: *const c_void,
            size: usize,
            position: u32,
            options: i32,
        ) -> i32;
    }
    let path = CString::new(path.as_os_str().as_bytes()).unwrap();
    let name = CString::new(name).unwrap();
    assert_eq!(
        unsafe {
            setxattr(
                path.as_ptr(),
                name.as_ptr(),
                value.as_ptr().cast(),
                value.len(),
                0,
                0,
            )
        },
        0,
        "{}",
        std::io::Error::last_os_error()
    );
}

#[test]
fn zero_bytes_are_a_verified_copy_not_an_unavailable_source() {
    let (root, mut request) = fixture();
    fs::write(root.path().join("source.txt"), b"").unwrap();
    fresh(&mut request);
    let receipt = run_request(&request);
    assert_eq!(receipt.status, MutationStatus::Completed);
    let verification = receipt.verification.unwrap();
    assert_eq!(verification.bytes, 0);
    assert_eq!(
        verification.content_sha256,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert!(fs::read(root.path().join("copy.txt")).unwrap().is_empty());
}

#[test]
fn permission_denial_is_known_not_started_not_a_completed_empty_copy() {
    let (root, mut request) = fixture();
    fs::set_permissions(root.path(), fs::Permissions::from_mode(0o500)).unwrap();
    fresh(&mut request);
    let receipt = run_request(&request);
    fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(receipt.status, MutationStatus::NotStarted);
    assert_eq!(receipt.destination_created, Some(false));
    assert_eq!(receipt.error.unwrap().code, FileErrorCode::PermissionDenied);
    assert!(!root.path().join("copy.txt").exists());
}

#[test]
fn a_real_finder_alias_is_not_resolved_or_copied() {
    use objc2_foundation::{NSString, NSURL, NSURLBookmarkCreationOptions};
    let (root, mut request) = fixture();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret"), "outside bytes").unwrap();
    let target = NSURL::fileURLWithPath(&NSString::from_str(
        outside.path().join("secret").to_str().unwrap(),
    ));
    let alias = NSURL::fileURLWithPath(&NSString::from_str(
        root.path().join("alias").to_str().unwrap(),
    ));
    let data = target
        .bookmarkDataWithOptions_includingResourceValuesForKeys_relativeToURL_error(
            NSURLBookmarkCreationOptions::SuitableForBookmarkFile,
            None,
            None,
        )
        .unwrap();
    NSURL::writeBookmarkData_toURL_options_error(&data, &alias, 0).unwrap();
    if let MutationAction::Copy { path, .. } = &mut request.action {
        *path = "alias".into();
    }
    fresh(&mut request);
    let original = fs::read(root.path().join("alias")).unwrap();
    assert_not_started(&request, FileErrorCode::UnsupportedType);
    assert_eq!(fs::read(root.path().join("alias")).unwrap(), original);
    assert_eq!(
        fs::read(outside.path().join("secret")).unwrap(),
        b"outside bytes"
    );
}
