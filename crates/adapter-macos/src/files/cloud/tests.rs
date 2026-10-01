use super::*;
use std::cell::Cell;
use std::fs;
use std::os::unix::fs::symlink;

struct Fixture {
    inspections: Cell<usize>,
    downloads: Cell<usize>,
    current: bool,
    downloading: bool,
    ubiquitous: Option<bool>,
    change_during_inspect: bool,
    replace_on_download: bool,
    download_error: Option<FileErrorCode>,
}
impl Default for Fixture {
    fn default() -> Self {
        Self {
            inspections: Cell::new(0),
            downloads: Cell::new(0),
            current: false,
            downloading: false,
            ubiquitous: Some(true),
            change_during_inspect: false,
            replace_on_download: false,
            download_error: None,
        }
    }
}
impl Native for Fixture {
    fn inspect(&self, file: &FileInfo) -> Result<CloudResources, FileError> {
        self.inspections.set(self.inspections.get() + 1);
        if self.change_during_inspect {
            fs::write(&file.path, "modified").unwrap();
        }
        let mut resources = native::skipped(native::Skip::Unavailable);
        resources.is_ubiquitous = self.ubiquitous.map_or(ResourceValue::Unavailable, |value| {
            ResourceValue::Available { value }
        });
        resources.downloading_status = ResourceValue::Available {
            value: if self.current {
                DownloadingStatus::Current
            } else {
                DownloadingStatus::NotDownloaded
            },
        };
        resources.is_downloading = ResourceValue::Available {
            value: self.downloading,
        };
        resources.download_requested = ResourceValue::Available { value: true };
        Ok(resources)
    }
    fn download(&self, file: &FileInfo) -> Result<(), FileError> {
        self.downloads.set(self.downloads.get() + 1);
        if self.replace_on_download {
            fs::remove_file(&file.path).unwrap();
            fs::create_dir(&file.path).unwrap();
        }
        if let Some(code) = self.download_error {
            return Err(FileError::new(code, "test native failure"));
        }
        Ok(())
    }
}

fn request(root: &Path) -> CloudRequest {
    CloudRequest {
        root: root.to_owned(),
        path: "a".into(),
        follow_links: false,
        expected_version: None,
        action: CloudAction::Status,
    }
}
fn downloadable(root: &Path) -> CloudRequest {
    let mut request = request(root);
    let file = observe(root, Path::new("a"), false, None).unwrap();
    request.expected_version = Some(file.version);
    request.action = CloudAction::Download {
        max_bytes: 32 * 1024 * 1024,
    };
    request
}
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a"), "unchanged </external> 臺灣").unwrap();
    dir
}

#[test]
fn status_keeps_partial_fields_and_never_submits_a_download() {
    let dir = fixture();
    let native = Fixture::default();
    let CloudResponse::CloudStatus(value) = execute(&request(dir.path()), &native, None).unwrap()
    else {
        panic!("status");
    };
    assert!(matches!(
        value.resources.is_ubiquitous,
        ResourceValue::Available { value: true }
    ));
    assert!(matches!(
        value.resources.is_uploaded,
        ResourceValue::Unavailable
    ));
    assert!(matches!(
        value.provider_coverage,
        ProviderCoverage::IcloudKeysOtherProvidersUnknown
    ));
    assert!(!value.download_requested_by_operation);
    assert_eq!(native.downloads.get(), 0);
    assert_eq!(
        fs::read_to_string(dir.path().join("a")).unwrap(),
        "unchanged </external> 臺灣"
    );
}

#[test]
fn explicit_download_submits_once_and_preserves_unverified_receipt() {
    let dir = fixture();
    let native = Fixture::default();
    let CloudResponse::CloudDownload(value) = execute(
        &downloadable(dir.path()),
        &native,
        Some(tempfile::tempdir().unwrap().path()),
    )
    .unwrap() else {
        panic!("download");
    };
    assert!(matches!(value.status, DownloadDisposition::SentUnverified));
    assert!(value.download_requested_by_operation);
    assert!(!value.completion_verified);
    assert!(value.post_check.is_ok());
    assert!(uuid::Uuid::parse_str(&value.operation_id).is_ok());
    assert_eq!(native.downloads.get(), 1);
    // Historical download_requested=true alone does not mean an active download.
    assert_eq!(
        fs::read_to_string(dir.path().join("a")).unwrap(),
        "unchanged </external> 臺灣"
    );
}

#[test]
fn current_or_active_downloads_return_no_submission_without_claiming_completion() {
    let dir = fixture();
    for (current, downloading) in [(true, false), (false, true)] {
        let native = Fixture {
            current,
            downloading,
            ..Default::default()
        };
        let CloudResponse::CloudDownload(value) = execute(
            &downloadable(dir.path()),
            &native,
            Some(tempfile::tempdir().unwrap().path()),
        )
        .unwrap() else {
            panic!("download");
        };
        assert!(matches!(
            (current, value.status),
            (true, DownloadDisposition::AlreadyCurrent)
                | (false, DownloadDisposition::AlreadyRequested)
        ));
        assert!(!value.download_requested_by_operation);
        assert!(!value.completion_verified);
        assert_eq!(native.downloads.get(), 0);
    }
}

#[test]
fn version_size_type_and_unknown_membership_block_native_submission() {
    let dir = fixture();
    let mut req = downloadable(dir.path());
    req.expected_version = Some("stale".into());
    let native = Fixture::default();
    assert_eq!(
        execute(&req, &native, Some(tempfile::tempdir().unwrap().path()))
            .unwrap_err()
            .code,
        FileErrorCode::Changed
    );
    req = downloadable(dir.path());
    req.action = CloudAction::Download { max_bytes: 1 };
    assert_eq!(
        execute(&req, &native, Some(tempfile::tempdir().unwrap().path()))
            .unwrap_err()
            .code,
        FileErrorCode::ScanLimit
    );
    assert_eq!(native.downloads.get(), 0);
    for ubiquitous in [None, Some(false)] {
        let native = Fixture {
            ubiquitous,
            ..Default::default()
        };
        assert_eq!(
            execute(
                &downloadable(dir.path()),
                &native,
                Some(tempfile::tempdir().unwrap().path())
            )
            .unwrap_err()
            .code,
            FileErrorCode::UnsupportedType
        );
        assert_eq!(native.downloads.get(), 0);
    }
}

#[test]
fn symlink_status_is_not_applicable_and_outside_traversal_is_rejected() {
    let dir = fixture();
    symlink("/outside/missing", dir.path().join("link")).unwrap();
    let native = Fixture::default();
    let mut req = request(dir.path());
    req.path = "link".into();
    let CloudResponse::CloudStatus(value) = execute(&req, &native, None).unwrap() else {
        panic!("status");
    };
    assert_eq!(value.file.kind, FileKind::Symlink);
    assert!(matches!(
        value.resources.is_ubiquitous,
        ResourceValue::NotApplicable
    ));
    assert_eq!(native.inspections.get(), 0);
    req.path = "../outside".into();
    assert_eq!(
        execute(&req, &native, None).unwrap_err().code,
        FileErrorCode::OutsideRoot
    );
}

#[test]
fn directory_and_unfollowed_link_downloads_are_rejected_without_submission() {
    let dir = fixture();
    fs::create_dir(dir.path().join("directory")).unwrap();
    symlink("a", dir.path().join("link")).unwrap();
    let native = Fixture::default();
    for path in ["directory", "link"] {
        let mut req = downloadable(dir.path());
        req.path = path.into();
        req.expected_version = Some(
            observe(dir.path(), Path::new(path), false, None)
                .unwrap()
                .version,
        );
        assert_eq!(
            execute(&req, &native, Some(tempfile::tempdir().unwrap().path()))
                .unwrap_err()
                .code,
            FileErrorCode::UnsupportedType
        );
        assert_eq!(native.downloads.get(), 0);
    }
    let mut req = downloadable(dir.path());
    req.path = "link".into();
    req.follow_links = true;
    let CloudResponse::CloudDownload(receipt) =
        execute(&req, &native, Some(tempfile::tempdir().unwrap().path())).unwrap()
    else {
        panic!("download");
    };
    assert_eq!(
        receipt.before.file.path,
        dir.path()
            .join("a")
            .canonicalize()
            .unwrap()
            .to_str()
            .unwrap()
    );
    assert_eq!(native.downloads.get(), 1);
}

#[test]
fn detected_changes_discard_status_but_preserve_already_submitted_download() {
    let dir = fixture();
    let native = Fixture {
        change_during_inspect: true,
        ..Default::default()
    };
    assert_eq!(
        execute(&request(dir.path()), &native, None)
            .unwrap_err()
            .code,
        FileErrorCode::Changed
    );
    let native = Fixture {
        replace_on_download: true,
        ..Default::default()
    };
    let CloudResponse::CloudDownload(value) = execute(
        &downloadable(dir.path()),
        &native,
        Some(tempfile::tempdir().unwrap().path()),
    )
    .unwrap() else {
        panic!("download");
    };
    assert!(matches!(value.status, DownloadDisposition::SentUnverified));
    assert!(value.download_requested_by_operation);
    assert_eq!(value.post_check.unwrap_err().code, FileErrorCode::Changed);
    assert_eq!(native.downloads.get(), 1);
}

#[test]
fn busy_operation_lock_blocks_submission_and_native_errors_do_not_retry() {
    let dir = fixture();
    let locks = tempfile::tempdir().unwrap();
    let held = crate::window::lock_path(&locks.path().join("files-cloud-download.lock")).unwrap();
    let native = Fixture::default();
    assert_eq!(
        execute(&downloadable(dir.path()), &native, Some(locks.path()))
            .unwrap_err()
            .code,
        FileErrorCode::Unavailable
    );
    assert_eq!(native.downloads.get(), 0);
    drop(held);
    let native = Fixture {
        download_error: Some(FileErrorCode::PermissionDenied),
        ..Default::default()
    };
    assert_eq!(
        execute(
            &downloadable(dir.path()),
            &native,
            Some(tempfile::tempdir().unwrap().path())
        )
        .unwrap_err()
        .code,
        FileErrorCode::PermissionDenied
    );
    assert_eq!(native.downloads.get(), 1);
}

#[test]
fn raw_worker_requests_require_download_revision_and_bounded_size() {
    let mut req = request(Path::new("/missing"));
    for max_bytes in [0, 1, 268435457] {
        req.action = CloudAction::Download { max_bytes };
        assert_eq!(
            execute(&req, &Fixture::default(), None).unwrap_err().code,
            FileErrorCode::InvalidOptions
        );
    }
    req.expected_version = Some("v".into());
    req.action = CloudAction::Download { max_bytes: 0 };
    assert_eq!(
        validate(&req).unwrap_err().code,
        FileErrorCode::InvalidOptions
    );
}

#[test]
fn worker_timeout_warns_that_a_download_may_continue() {
    use std::os::unix::fs::PermissionsExt;
    let dir = fixture();
    let exe = dir.path().join("worker");
    fs::write(&exe, "#!/bin/sh\nexec /bin/sleep 30\n").unwrap();
    fs::set_permissions(&exe, fs::Permissions::from_mode(0o700)).unwrap();
    let error = run(&exe, &downloadable(dir.path()), 100).unwrap_err();
    assert_eq!(error.code, FileErrorCode::Timeout);
    assert!(error.message.contains("may already have been requested"));
}

#[test]
fn dataless_current_flags_do_not_claim_download_already_current() {
    let dir = fixture();
    let CloudResponse::CloudStatus(mut value) = execute(
        &request(dir.path()),
        &Fixture {
            current: true,
            ..Default::default()
        },
        None,
    )
    .unwrap() else {
        panic!("status");
    };
    value.file.data_state = DataState::Dataless;
    assert!(preflight(&value, 33554432).unwrap().is_none());
}
