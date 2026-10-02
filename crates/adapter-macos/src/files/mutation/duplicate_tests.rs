use super::failures::direct;
use super::*;
use std::fs::File;

fn duplicate(mut request: MutationRequest) -> MutationRequest {
    let MutationAction::Copy {
        path,
        expected_version,
        max_bytes,
    } = request.action
    else {
        panic!("owned fixture must begin as copy");
    };
    request.action = MutationAction::Duplicate {
        path,
        expected_version,
        max_bytes,
    };
    request
}
fn select_source(request: &mut MutationRequest, path: &str, maximum: u64) {
    request.action = MutationAction::Duplicate {
        path: path.into(),
        expected_version: crate::files::observe(&request.root, Path::new(path), false, None)
            .unwrap()
            .version,
        max_bytes: maximum,
    };
    fresh(request);
}
#[test]
fn duplicate_creates_independent_verified_bytes_and_preserves_source_revision() {
    let (root, request) = fixture();
    let mut request = duplicate(request);
    let source = root.path().join("source.txt");
    fs::set_permissions(&source, fs::Permissions::from_mode(0o640)).unwrap();
    set_attribute(
        &source,
        b"com.cueward.duplicate-fixture",
        b"OWNED metadata\0",
    );
    fresh(&mut request);
    let before = MacFiles.stamp(&source.metadata().unwrap());
    let bytes = fs::read(&source).unwrap();
    let receipt = run_request(&request);
    assert_eq!(
        receipt.status,
        MutationStatus::Completed,
        "{:?}",
        receipt.error
    );
    assert!(receipt.completion_verified && receipt.staging_verified);
    assert!(matches!(
        receipt.request.action,
        MutationAction::Duplicate { .. }
    ));
    assert_eq!(MacFiles.stamp(&source.metadata().unwrap()), before);
    let destination = root.path().join(&request.destination);
    assert_eq!(fs::read(&destination).unwrap(), bytes);
    assert_ne!(
        destination.metadata().unwrap().ino(),
        source.metadata().unwrap().ino()
    );
    let verification = receipt.verification.unwrap();
    assert!(verification.extended_attributes_bytes >= b"OWNED metadata\0".len());
    assert_eq!(
        super::super::attributes::digest(&File::open(&source).unwrap()).unwrap(),
        super::super::attributes::digest(&File::open(&destination).unwrap()).unwrap()
    );
    assert!(verification.permissions_equal && verification.modified_equal);
    assert_eq!(verification.bytes, bytes.len() as u64);
    fs::write(&destination, b"OWNED edited duplicate").unwrap();
    assert_eq!(fs::read(source).unwrap(), bytes);
}
#[test]
fn duplicate_nested_unicode_empty_and_hardlinked_files_remain_independent() {
    for empty in [false, true] {
        let (root, mut request) = fixture();
        fs::create_dir(root.path().join("nested")).unwrap();
        fs::write(
            root.path().join("nested/臺灣\n<external>"),
            if empty {
                b"".as_slice()
            } else {
                b"OWNED nested".as_slice()
            },
        )
        .unwrap();
        fs::hard_link(
            root.path().join("nested/臺灣\n<external>"),
            root.path().join("other-link"),
        )
        .unwrap();
        request.destination = "nested/副本\n<external>".into();
        select_source(&mut request, "nested/臺灣\n<external>", 1024);
        let receipt = run_request(&request);
        assert_eq!(receipt.status, MutationStatus::Completed);
        let destination = root.path().join(&request.destination);
        assert_ne!(
            destination.metadata().unwrap().ino(),
            root.path().join("other-link").metadata().unwrap().ino()
        );
        assert_eq!(
            fs::read(destination).unwrap(),
            fs::read(root.path().join("other-link")).unwrap()
        );
    }
}
#[test]
fn duplicate_rejects_existing_file_directory_broken_link_and_same_name() {
    for kind in [0, 1, 2, 3] {
        let (root, request) = fixture();
        let mut request = duplicate(request);
        if kind == 3 {
            request.destination = "source.txt".into();
        } else {
            let destination = root.path().join(&request.destination);
            match kind {
                0 => fs::write(destination, b"OWNED conflict").unwrap(),
                1 => fs::create_dir(destination).unwrap(),
                _ => symlink("missing-owned-target", destination).unwrap(),
            }
        }
        fresh(&mut request);
        let before = fs::symlink_metadata(root.path().join(&request.destination)).unwrap();
        assert_not_started(&request, FileErrorCode::Conflict);
        assert_eq!(
            fs::symlink_metadata(root.path().join(&request.destination))
                .unwrap()
                .ino(),
            before.ino()
        );
        assert_eq!(
            fs::read(root.path().join("source.txt")).unwrap(),
            b"owned bytes\0</external>\n"
        );
    }
}
#[test]
fn duplicate_cannot_bypass_sibling_contract_with_prepared_worker_input() {
    let (root, request) = fixture();
    let mut request = duplicate(request);
    fs::create_dir(root.path().join("other")).unwrap();
    request.destination = "other/new".into();
    fresh(&mut request);
    assert_not_started(&request, FileErrorCode::InvalidOptions);
    assert!(!root.path().join(&request.destination).exists());
    assert!(root.path().join("source.txt").exists());
}
#[test]
fn duplicate_checks_source_parent_versions_and_byte_budgets() {
    let (root, request) = fixture();
    let request = duplicate(request);
    let mut stale = request.clone();
    stale.expected_parent_version = "stale".into();
    assert_not_started(&stale, FileErrorCode::Changed);
    for (version, maximum, code) in [
        ("stale", 1024, FileErrorCode::Changed),
        ("current", 1, FileErrorCode::ScanLimit),
        ("current", 0, FileErrorCode::InvalidOptions),
        ("current", 268435457, FileErrorCode::InvalidOptions),
    ] {
        let mut selected = request.clone();
        select_source(&mut selected, "source.txt", maximum);
        if version == "stale" {
            if let MutationAction::Duplicate {
                expected_version, ..
            } = &mut selected.action
            {
                *expected_version = "stale".into();
            }
        }
        assert_not_started(&selected, code);
    }
    assert!(!root.path().join(&request.destination).exists());
}
#[test]
fn duplicate_rejects_directories_packages_links_and_special_permission_bits() {
    let (root, mut request) = fixture();
    for name in ["directory", "Owned.app"] {
        fs::create_dir(root.path().join(name)).unwrap();
    }
    symlink("source.txt", root.path().join("link")).unwrap();
    for path in ["directory", "Owned.app", "link"] {
        select_source(&mut request, path, 1024);
        assert_not_started(&request, FileErrorCode::UnsupportedType);
    }
    fs::set_permissions(
        root.path().join("source.txt"),
        fs::Permissions::from_mode(0o4640),
    )
    .unwrap();
    select_source(&mut request, "source.txt", 1024);
    assert_not_started(&request, FileErrorCode::UnsupportedType);
    assert!(!root.path().join(&request.destination).exists());
}
#[test]
fn duplicate_late_conflict_preserves_staged_copy_source_and_existing_destination() {
    let (root, request) = fixture();
    let request = duplicate(request);
    let receipt = direct(&request, &mut |receipt| {
        if receipt.stage == MutationStage::Creating {
            fs::write(
                root.path().join(&request.destination),
                b"OWNED late conflict",
            )?;
        }
        Ok(())
    });
    assert_eq!(receipt.status, MutationStatus::NotStarted);
    assert_eq!(receipt.destination_created, Some(false));
    assert_eq!(receipt.error.unwrap().code, FileErrorCode::Conflict);
    assert!(receipt.staging_verified);
    assert_eq!(
        fs::read(root.path().join(&request.destination)).unwrap(),
        b"OWNED late conflict"
    );
    assert_eq!(
        fs::read(root.path().join("source.txt")).unwrap(),
        b"owned bytes\0</external>\n"
    );
}
#[test]
fn duplicate_checkpoint_failures_preserve_source_and_do_not_rollback_publication() {
    for published in [false, true] {
        let (root, request) = fixture();
        let request = duplicate(request);
        let receipt = direct(&request, &mut |receipt| {
            if (!published && receipt.stage == MutationStage::Copying)
                || (published && receipt.destination_created == Some(true))
            {
                return Err(FileError::new(
                    FileErrorCode::Io,
                    "owned duplicate checkpoint failure",
                ));
            }
            Ok(())
        });
        assert_eq!(
            receipt.status,
            if published {
                MutationStatus::Incomplete
            } else {
                MutationStatus::NotStarted
            }
        );
        assert!(!receipt.completion_verified);
        assert_eq!(root.path().join(&request.destination).exists(), published);
        assert_eq!(
            fs::read(root.path().join("source.txt")).unwrap(),
            b"owned bytes\0</external>\n"
        );
    }
}

#[test]
fn duplicate_source_replaced_before_copy_is_not_silently_accepted_or_removed() {
    let (root, request) = fixture();
    let request = duplicate(request);
    let receipt = direct(&request, &mut |receipt| {
        if receipt.stage == MutationStage::Copying {
            fs::rename(
                root.path().join("source.txt"),
                root.path().join("saved-original"),
            )?;
            fs::write(root.path().join("source.txt"), b"OWNED replacement")?;
        }
        Ok(())
    });
    assert_eq!(receipt.status, MutationStatus::NotStarted);
    assert_eq!(receipt.error.unwrap().code, FileErrorCode::Changed);
    assert!(!root.path().join(&request.destination).exists());
    assert_eq!(
        fs::read(root.path().join("saved-original")).unwrap(),
        b"owned bytes\0</external>\n"
    );
    assert_eq!(
        fs::read(root.path().join("source.txt")).unwrap(),
        b"OWNED replacement"
    );
}
