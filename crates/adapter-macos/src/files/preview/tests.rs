use super::*;
use std::fs;
use std::os::unix::fs::symlink;

fn request(root: &Path) -> PreviewRequest {
    PreviewRequest {
        root: root.to_owned(),
        path: "a.pdf".into(),
        follow_links: false,
        expected_version: None,
        options: PreviewOptions {
            kind: PreviewKind::Pdf,
            start_page: 1,
            page_count: 10,
            ocr: false,
            render: true,
            max_input_bytes: 64 * 1024 * 1024,
            max_text_bytes: 65536,
            max_dimension: 1024,
            max_preview_bytes: 8 * 1024 * 1024,
        },
    }
}
fn fixture() -> (tempfile::TempDir, tempfile::TempDir, PreviewWorkerRequest) {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("a.pdf"), "guarded </external> bytes").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let worker = PreviewWorkerRequest {
        request: request(root.path()),
        directory: dir.path().to_owned(),
    };
    (root, dir, worker)
}
fn no_previews(_: &Path, _: &Path, _: &PreviewOptions) -> Result<native::NativePreview, FileError> {
    Ok(native::NativePreview {
        total_pages: Some(0),
        pdf_encrypted: Some(false),
        image: None,
        pages: vec![],
        selection_truncated: false,
    })
}

#[test]
fn preview_snapshot_preserves_bytes_hash_and_removes_private_input_on_success() {
    use sha2::{Digest, Sha256};
    let (root, dir, worker) = fixture();
    let contents = fs::read(root.path().join("a.pdf")).unwrap();
    let response = execute(&worker, |input, _, _| {
        assert_eq!(fs::read(input).unwrap(), contents);
        no_previews(input, dir.path(), &worker.request.options)
    })
    .unwrap();
    let PreviewResponse::Preview(value) = response;
    assert_eq!(
        value.source_sha256,
        format!("{:x}", Sha256::digest(&contents))
    );
    assert_eq!(fs::read(root.path().join("a.pdf")).unwrap(), contents);
    assert!(fs::read_dir(dir.path()).unwrap().next().is_none());
    assert!(value.cache_directory.is_none());
}
#[test]
fn source_change_during_native_extraction_discards_preview() {
    let (root, _directory, worker) = fixture();
    assert_eq!(
        execute(&worker, |input, dir, options| {
            fs::write(root.path().join("a.pdf"), "changed").unwrap();
            no_previews(input, dir, options)
        })
        .unwrap_err()
        .code,
        FileErrorCode::Changed
    );
}
#[test]
fn size_version_and_scope_reject_before_native_extraction() {
    let (_root, _directory, mut worker) = fixture();
    worker.request.expected_version = Some("stale".into());
    assert_eq!(
        execute(&worker, |_, _, _| panic!("native must not run"))
            .unwrap_err()
            .code,
        FileErrorCode::Changed
    );
    worker.request.expected_version = None;
    worker.request.options.max_input_bytes = 1;
    assert_eq!(
        execute(&worker, |_, _, _| panic!("native must not run"))
            .unwrap_err()
            .code,
        FileErrorCode::ScanLimit
    );
    worker.request.path = "../outside".into();
    assert_eq!(
        execute(&worker, |_, _, _| panic!("native must not run"))
            .unwrap_err()
            .code,
        FileErrorCode::OutsideRoot
    );
}
#[test]
fn directory_and_links_do_not_enter_native_hook() {
    let (root, _directory, mut worker) = fixture();
    fs::create_dir(root.path().join("folder")).unwrap();
    symlink("a.pdf", root.path().join("link")).unwrap();
    for name in ["folder", "link"] {
        worker.request.path = name.into();
        assert_eq!(
            execute(&worker, |_, _, _| panic!("native must not run"))
                .unwrap_err()
                .code,
            FileErrorCode::UnsupportedType
        );
    }
    worker.request.path = "link".into();
    worker.request.follow_links = true;
    assert!(execute(&worker, no_previews).is_ok());
}
#[test]
fn raw_preview_bounds_and_cache_directory_are_checked_before_work() {
    let (_root, dir, mut worker) = fixture();
    worker.request.options.max_dimension = 2049;
    assert_eq!(
        validate(&worker.request).unwrap_err().code,
        FileErrorCode::InvalidOptions
    );
    worker.request.options.max_dimension = 1024;
    worker.request.options.kind = PreviewKind::Thumbnail;
    worker.request.options.page_count = 1;
    worker.request.options.ocr = true;
    assert_eq!(
        validate(&worker.request).unwrap_err().code,
        FileErrorCode::InvalidOptions
    );
    assert_eq!(
        validate_directory(dir.path()).unwrap_err().code,
        FileErrorCode::InvalidOptions
    );
}
#[test]
fn malformed_native_page_or_escaping_artifact_is_rejected() {
    let (_root, dir, worker) = fixture();
    let mut result = no_previews(Path::new(""), dir.path(), &worker.request.options).unwrap();
    result.pages.push(PagePreview {
        page: 7,
        text: ResourceValue::Available {
            value: "text".into(),
        },
        text_source: TextSource::NativePdf,
        confidence: None,
        text_truncated: false,
        preview: ResourceValue::NotApplicable,
    });
    assert_eq!(
        artifacts::verify(&mut result, dir.path(), &worker.request.options)
            .unwrap_err()
            .code,
        FileErrorCode::Internal
    );
    result.pages[0].page = 1;
    result.pages[0].preview = ResourceValue::Available {
        value: PreviewArtifact {
            path: "../outside.png".into(),
            format: "png".into(),
            width: 10,
            height: 10,
            bytes: 100,
            sha256: String::new(),
        },
    };
    assert_eq!(
        artifacts::verify(&mut result, dir.path(), &worker.request.options)
            .unwrap_err()
            .code,
        FileErrorCode::Internal
    );
}
#[test]
fn dataless_observation_is_not_treated_as_an_empty_source() {
    let (root, dir, _worker) = fixture();
    let mut file = observe(root.path(), Path::new("a.pdf"), false, None).unwrap();
    file.data_state = DataState::Dataless;
    assert_eq!(
        snapshot::create(&file, dir.path(), 1024).unwrap_err().code,
        FileErrorCode::Unavailable
    );
    assert!(fs::read_dir(dir.path()).unwrap().next().is_none());
}
#[test]
fn preview_worker_timeout_cleans_up_private_snapshot_directory() {
    use std::os::unix::fs::PermissionsExt;
    let (root, _directory, worker) = fixture();
    let exe = root.path().join("worker");
    let record = root.path().join("request.json");
    let quoted = record.to_str().unwrap().replace('\'', "'\\''");
    fs::write(
        &exe,
        format!("#!/bin/sh\n/bin/cat > '{quoted}'\nexec /bin/sleep 30\n"),
    )
    .unwrap();
    fs::set_permissions(&exe, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        run(&exe, &worker.request, 1000).unwrap_err().code,
        FileErrorCode::Timeout
    );
    let value: serde_json::Value = serde_json::from_slice(&fs::read(record).unwrap()).unwrap();
    assert!(!Path::new(value["directory"].as_str().unwrap()).exists());
}

#[test]
fn thumbnail_rejects_a_real_alias_without_resolving_its_outside_target() {
    use objc2_foundation::{NSString, NSURL, NSURLBookmarkCreationOptions};
    let (root, _directory, mut worker) = fixture();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret.txt"), "outside secret").unwrap();
    let target = NSURL::fileURLWithPath(&NSString::from_str(
        outside.path().join("secret.txt").to_str().unwrap(),
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
    worker.request.path = "alias".into();
    worker.request.options.kind = PreviewKind::Thumbnail;
    worker.request.options.page_count = 1;
    let before = fs::read(root.path().join("alias")).unwrap();
    assert_eq!(
        execute(&worker, |_, _, _| panic!("must not enter Quick Look"))
            .unwrap_err()
            .code,
        FileErrorCode::UnsupportedType
    );
    assert_eq!(fs::read(root.path().join("alias")).unwrap(), before);
    assert_eq!(
        fs::read(outside.path().join("secret.txt")).unwrap(),
        b"outside secret"
    );
}
#[test]
fn operation_cache_is_private_and_rejects_reuse_with_existing_files() {
    use std::os::unix::fs::MetadataExt;
    let dir = operation_directory().unwrap();
    assert_eq!(fs::metadata(dir.path()).unwrap().mode() & 0o777, 0o700);
    validate_directory(dir.path()).unwrap();
    fs::write(dir.path().join("existing"), "must not overwrite").unwrap();
    assert_eq!(
        validate_directory(dir.path()).unwrap_err().code,
        FileErrorCode::InvalidOptions
    );
}

#[test]
fn native_toolchain_failure_is_unavailable_and_invalid_json_remains_internal() {
    use std::os::unix::process::ExitStatusExt;
    let mut output = std::process::Output {
        status: std::process::ExitStatus::from_raw(1 << 8),
        stdout: vec![],
        stderr: b"missing toolchain".to_vec(),
    };
    let error = native::parse(&output).unwrap_err();
    assert_eq!(error.code, FileErrorCode::Unavailable);
    assert!(error.message.contains("missing toolchain"));
    output.status = std::process::ExitStatus::from_raw(0);
    output.stdout = b"invalid".to_vec();
    assert_eq!(
        native::parse(&output).unwrap_err().code,
        FileErrorCode::Internal
    );
}
