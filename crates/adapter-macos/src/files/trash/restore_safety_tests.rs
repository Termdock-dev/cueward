use super::super::execution::tests as trash;
use super::tests::*;
use super::*;
use std::fs;
use std::os::unix::fs::symlink;
#[test]
fn restore_refuses_incomplete_missing_inconsistent_and_unverified_original_evidence_before_copy() {
    for mode in 0..15 {
        let mut value = fixture();
        match mode {
            0 => value.original.receipt.status = MutationStatus::Incomplete,
            1 => value.original.receipt.completion_verified = false,
            2 => value.original.receipt.backup_verified = false,
            3 => value.original.receipt.source_removed = None,
            4 => value.original.receipt.moved_to_trash = None,
            5 => value.original.receipt.backup_verification = None,
            6 => value.original.receipt.backup_after = None,
            7 => value.original.receipt.root_before = None,
            8 => value.original.receipt.parent_before = None,
            9 => value.original.receipt.source_before = None,
            10 => value.original.receipt.backup_path = Some("/other/staged-object".into()),
            11 => {
                value
                    .original
                    .receipt
                    .backup_verification
                    .as_mut()
                    .unwrap()
                    .content_sha256 = "not-sha256".into()
            }
            12 => {
                value
                    .original
                    .receipt
                    .backup_verification
                    .as_mut()
                    .unwrap()
                    .bytes += 1
            }
            13 => value.original.receipt.stage = super::super::execution::TrashStage::Trashing,
            14 => {
                value.receipt.request.trash_operation_id =
                    "00000000-0000-0000-0000-000000000000".into()
            }
            _ => unreachable!(),
        }
        direct(&mut value, |_| Ok(()));
        assert_eq!(
            value.receipt.status,
            MutationStatus::NotStarted,
            "mode={mode} {:?}",
            value.receipt.error
        );
        assert!(!value.receipt.mutation_attempted && value.receipt.staging_path.is_none());
        assert!(!value.original.root.path().join("source").exists());
    }
}
#[test]
fn restore_never_overwrites_existing_file_directory_or_broken_link() {
    for mode in 0..3 {
        let mut value = fixture();
        let path = value.original.root.path().join("source");
        match mode {
            0 => fs::write(&path, b"OWNED conflict").unwrap(),
            1 => fs::create_dir(&path).unwrap(),
            2 => symlink("missing-owned", &path).unwrap(),
            _ => unreachable!(),
        }
        value.receipt.request.expected_parent_version =
            request(&value.original).expected_parent_version;
        let original = original_json(&value);
        direct(&mut value, |_| Ok(()));
        assert_eq!(value.receipt.status, MutationStatus::NotStarted);
        assert!(!value.receipt.mutation_attempted && value.receipt.staging_path.is_none());
        match mode {
            0 => assert_eq!(fs::read(&path).unwrap(), b"OWNED conflict"),
            1 => assert!(path.is_dir()),
            2 => assert_eq!(fs::read_link(&path).unwrap(), Path::new("missing-owned")),
            _ => unreachable!(),
        }
        assert_retained(&value, &original);
    }
}
#[test]
fn restore_refuses_stale_parent_wrong_root_and_changed_backup_data_identity_or_attributes() {
    for mode in 0..6 {
        let mut value = fixture();
        let other = tempfile::tempdir().unwrap();
        let backup = Path::new(value.original.receipt.backup_path.as_ref().unwrap());
        match mode {
            0 => value.receipt.request.expected_parent_version = "stale".into(),
            1 => value.receipt.request.root = other.path().into(),
            2 => fs::write(backup, b"OWNED changed backup").unwrap(),
            3 => {
                let bytes = fs::read(backup).unwrap();
                fs::remove_file(backup).unwrap();
                fs::write(backup, bytes).unwrap();
            }
            4 => {
                fs::remove_file(backup).unwrap();
                symlink(trash::trashed(&value.original), backup).unwrap();
            }
            5 => assert!(
                std::process::Command::new("/usr/bin/xattr")
                    .args(["-w", "com.cueward.restore-owned", "change"])
                    .arg(backup)
                    .status()
                    .unwrap()
                    .success()
            ),
            _ => unreachable!(),
        }
        direct(&mut value, |_| Ok(()));
        assert_eq!(
            value.receipt.status,
            MutationStatus::NotStarted,
            "mode={mode} {:?}",
            value.receipt.error
        );
        assert!(value.receipt.staging_path.is_none());
        assert!(!value.original.root.path().join("source").exists());
        assert_eq!(
            fs::read(trash::trashed(&value.original)).unwrap(),
            b"OWNED bytes\0</external>"
        );
    }
}
#[test]
fn restore_revalidates_backup_staging_and_destination_at_each_publication_boundary() {
    for mode in 0..4 {
        let mut value = fixture();
        let root = value.original.root.path().to_owned();
        let backup = value.original.receipt.backup_path.clone().unwrap();
        direct(&mut value, |r| {
            if r.stage == RestoreStage::Publishing {
                match mode {
                    0 => fs::write(&backup, b"OWNED backup changed").unwrap(),
                    1 => fs::write(r.staging_path.as_ref().unwrap(), b"OWNED staging changed")
                        .unwrap(),
                    2 => {
                        fs::write(root.join("source"), b"OWNED conflict").unwrap();
                    }
                    3 => {
                        fs::rename(&root, root.with_extension("moved")).unwrap();
                        fs::create_dir(&root).unwrap();
                    }
                    _ => unreachable!(),
                }
            }
            Ok(())
        });
        assert_eq!(
            value.receipt.status,
            MutationStatus::NotStarted,
            "mode={mode} {:?}",
            value.receipt.error
        );
        assert!(!value.receipt.completion_verified);
        assert_eq!(value.receipt.destination_created, Some(false));
        if mode == 2 {
            assert_eq!(fs::read(root.join("source")).unwrap(), b"OWNED conflict");
        } else {
            assert!(!root.join("source").exists());
        }
        if mode == 3 {
            fs::remove_dir(&root).unwrap();
            fs::rename(root.with_extension("moved"), &root).unwrap();
        }
        assert_eq!(
            fs::read(trash::trashed(&value.original)).unwrap(),
            b"OWNED bytes\0</external>"
        );
    }
}
#[test]
fn restore_postpublication_change_is_incomplete_without_repair_or_cleanup() {
    let mut value = fixture();
    let original = original_json(&value);
    let path = value.original.root.path().join("source");
    direct(&mut value, |r| {
        if r.stage == RestoreStage::Verifying {
            fs::write(&path, b"OWNED later edit").unwrap();
        }
        Ok(())
    });
    assert_eq!(value.receipt.status, MutationStatus::Incomplete);
    assert_eq!(value.receipt.destination_created, Some(true));
    assert!(!value.receipt.completion_verified);
    assert_eq!(fs::read(path).unwrap(), b"OWNED later edit");
    assert_retained(&value, &original);
}
#[test]
fn restore_adapter_rejects_changed_original_receipt_while_copying() {
    let value = fixture();
    let mut platform = trash::platform(&value.original);
    let original_path = value.original.receipt.receipt_path.clone();
    // The copy hook changes only this test-owned original receipt after preflight.
    let control = tempfile::tempdir().unwrap();
    platform.pause = Some(control.path().to_owned());
    std::thread::scope(|scope| {
        scope.spawn(|| {
            super::lifetime_tests::wait_for(|| control.path().join("ready").exists());
            let mut original: serde_json::Value =
                serde_json::from_slice(&fs::read(&original_path).unwrap()).unwrap();
            original["recovery_supported"] = serde_json::json!(true);
            fs::write(
                &original_path,
                serde_json::to_vec_pretty(&original).unwrap(),
            )
            .unwrap();
            fs::write(control.path().join("release"), b"").unwrap();
        });
        let receipt = execute_prepared(
            &platform,
            &RestoreWorkerRequest {
                operation_id: value.receipt.operation_id.clone(),
            },
        )
        .unwrap();
        assert_eq!(
            receipt.status,
            MutationStatus::NotStarted,
            "{:?}",
            receipt.error
        );
        assert_eq!(receipt.error.unwrap().code, FileErrorCode::Changed);
        assert!(!receipt.mutation_attempted && receipt.staging_verified);
        assert!(!value.original.root.path().join("source").exists());
    });
}
fn nested_fixture() -> Owned {
    let mut original = trash::fixture();
    fs::create_dir(original.root.path().join("parent")).unwrap();
    fs::rename(
        original.root.path().join("source"),
        original.root.path().join("parent/source"),
    )
    .unwrap();
    original.receipt.request.path = "parent/source".into();
    original.receipt.request.expected_version = crate::files::observe(
        original.root.path(),
        Path::new("parent/source"),
        false,
        None,
    )
    .unwrap()
    .version;
    original.receipt.request.expected_parent_version =
        crate::files::observe(original.root.path(), Path::new("parent"), false, None)
            .unwrap()
            .version;
    from_original(original)
}
#[test]
fn restore_never_recreates_missing_parents_or_accepts_replaced_original_parent() {
    for replace in [false, true] {
        let mut value = nested_fixture();
        let parent = value.original.root.path().join("parent");
        fs::remove_dir(&parent).unwrap();
        if replace {
            fs::create_dir(&parent).unwrap();
            value.receipt.request.expected_parent_version =
                crate::files::observe(value.original.root.path(), Path::new("parent"), false, None)
                    .unwrap()
                    .version;
        }
        direct(&mut value, |_| Ok(()));
        assert_eq!(value.receipt.status, MutationStatus::NotStarted);
        assert!(value.receipt.staging_path.is_none());
        assert!(!parent.join("source").exists());
        if !replace {
            assert!(!parent.exists());
        }
    }
}
#[test]
fn restore_guarded_publication_does_not_follow_parent_retargeted_after_last_precheck() {
    let mut value = nested_fixture();
    let original = original_json(&value);
    let outside = tempfile::tempdir().unwrap();
    let parent = value.original.root.path().join("parent");
    let moved = outside.path().join("moved-parent");
    let destination = outside.path().to_owned();
    let mut platform = trash::platform(&value.original);
    platform.race = Some(Box::new(move || {
        fs::rename(&parent, &moved).unwrap();
        symlink(&destination, &parent).unwrap();
    }));
    direct_with(&mut value, &platform, &mut |_| Ok(()));
    assert_eq!(
        value.receipt.status,
        MutationStatus::NotStarted,
        "{:?}",
        value.receipt.error
    );
    assert_eq!(value.receipt.destination_created, Some(false));
    assert!(!outside.path().join("source").exists());
    assert!(!outside.path().join("moved-parent/source").exists());
    assert!(Path::new(value.receipt.staging_path.as_ref().unwrap()).is_file());
    assert_retained(&value, &original);
}
#[test]
fn restore_exclusive_publication_preserves_collision_created_after_last_precheck() {
    let mut value = fixture();
    let original = original_json(&value);
    let path = value.original.root.path().join("source");
    let target = path.clone();
    let mut platform = trash::platform(&value.original);
    platform.race = Some(Box::new(move || {
        fs::write(&target, b"OWNED late collision").unwrap()
    }));
    direct_with(&mut value, &platform, &mut |_| Ok(()));
    assert_eq!(
        value.receipt.status,
        MutationStatus::NotStarted,
        "{:?}",
        value.receipt.error
    );
    assert_eq!(value.receipt.destination_created, Some(false));
    assert_eq!(fs::read(path).unwrap(), b"OWNED late collision");
    assert_retained(&value, &original);
}

#[test]
fn restore_oversized_original_observations_are_omitted_before_staging() {
    let mut value = fixture();
    value
        .original
        .receipt
        .source_before
        .as_mut()
        .unwrap()
        .requested_path = "x".repeat(64 * 1024);
    direct(&mut value, |_| Ok(()));
    assert_eq!(value.receipt.status, MutationStatus::NotStarted);
    assert_eq!(
        value.receipt.error.as_ref().unwrap().code,
        FileErrorCode::ScanLimit
    );
    assert!(value.receipt.preflight_observations_omitted);
    assert!(
        value.receipt.original_source.is_none()
            && value.receipt.root_before.is_none()
            && value.receipt.parent_before.is_none()
            && value.receipt.backup_before.is_none()
    );
    assert!(value.receipt.staging_path.is_none());
    assert!(!value.original.root.path().join("source").exists());
    assert!(serde_json::to_vec_pretty(&value.receipt).unwrap().len() < 64 * 1024);
}
