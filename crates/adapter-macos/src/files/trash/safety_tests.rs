use super::tests::*;
use super::*;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::PathBuf;

#[test]
fn trash_execute_pre_move_changes_and_checkpoint_failure_leave_original_and_backup() {
    for mode in 0..4 {
        let mut value = fixture();
        let platform = platform(&value);
        let source = value.root.path().join("source");
        let trash = value.trash.path().to_owned();
        direct(&mut value, &platform, |r| {
            if r.stage == TrashStage::BackedUp {
                match mode {
                    0 => fs::write(&source, b"OWNED external change").unwrap(),
                    1 => fs::write(r.backup_path.as_ref().unwrap(), b"OWNED backup edit").unwrap(),
                    2 => fs::write(trash.join("owned-conflict"), b"OWNED conflict").unwrap(),
                    3 => {
                        return Err(FileError::new(
                            FileErrorCode::Io,
                            "owned checkpoint failed before removal",
                        ));
                    }
                    _ => unreachable!(),
                }
            }
            Ok(())
        });
        assert_eq!(value.receipt.status, MutationStatus::NotStarted);
        assert!(!value.receipt.completion_verified);
        assert!(source.exists());
        if mode != 1 {
            assert_eq!(backup(&value), b"OWNED bytes\0</external>");
        }
        if mode == 2 {
            assert_eq!(
                fs::read(trash.join("owned-conflict")).unwrap(),
                b"OWNED conflict"
            );
        }
    }
}
#[test]
fn trash_execute_kernel_move_never_removes_from_cached_parent_moved_outside_selected_root() {
    for replacement in [false, true] {
        let outside = tempfile::tempdir().unwrap();
        let mut value = fixture();
        fs::create_dir(value.root.path().join("parent")).unwrap();
        fs::rename(
            value.root.path().join("source"),
            value.root.path().join("parent/source"),
        )
        .unwrap();
        value.receipt.request.path = "parent/source".into();
        value.receipt.request.expected_version =
            crate::files::observe(value.root.path(), Path::new("parent/source"), false, None)
                .unwrap()
                .version;
        value.receipt.request.expected_parent_version =
            crate::files::observe(value.root.path(), Path::new("parent"), false, None)
                .unwrap()
                .version;
        let root = value.root.path().to_owned();
        let displaced = outside.path().join("moved-parent");
        let target = displaced.clone();
        let mut platform = platform(&value);
        platform.race = Some(Box::new(move || {
            fs::rename(root.join("parent"), &target).unwrap();
            if replacement {
                symlink(&target, root.join("parent")).unwrap();
            }
        }));
        direct(&mut value, &platform, |_| Ok(()));
        assert_eq!(value.receipt.status, MutationStatus::NotStarted);
        assert_eq!(value.receipt.source_removed, Some(false));
        assert!(!value.receipt.trash_attempted);
        assert_eq!(
            fs::read(displaced.join("source")).unwrap(),
            b"OWNED bytes\0</external>"
        );
        assert_eq!(backup(&value), b"OWNED bytes\0</external>");
        assert!(value.receipt.trash_path.is_none());
    }
}
#[test]
fn trash_execute_same_path_replacement_at_submission_is_incomplete_without_deleting_either_object()
{
    let mut value = fixture();
    let root = value.root.path().to_owned();
    let mut platform = platform(&value);
    platform.race = Some(Box::new(move || {
        fs::rename(root.join("source"), root.join("displaced-owned-original")).unwrap();
        fs::write(root.join("source"), b"OWNED replacement").unwrap();
    }));
    direct(&mut value, &platform, |_| Ok(()));
    assert_eq!(value.receipt.status, MutationStatus::Incomplete);
    assert_eq!(value.receipt.source_removed, Some(true));
    assert!(!value.receipt.trash_attempted && !value.receipt.completion_verified);
    assert_eq!(
        fs::read(value.receipt.staging_path.as_ref().unwrap()).unwrap(),
        b"OWNED replacement"
    );
    assert_eq!(backup(&value), b"OWNED bytes\0</external>");
    assert_eq!(
        fs::read(value.root.path().join("displaced-owned-original")).unwrap(),
        backup(&value)
    );
    assert_ne!(
        value.receipt.source_before.as_ref().unwrap().identity,
        value.receipt.staged_after.as_ref().unwrap().identity
    );
}
#[test]
fn trash_execute_post_move_changes_are_incomplete_and_never_overwrite_or_repair() {
    for mode in 0..3 {
        let mut value = fixture();
        let platform = platform(&value);
        let root = value.root.path().to_owned();
        direct(&mut value, &platform, |r| {
            if r.stage == TrashStage::Verifying {
                match mode {
                    0 => fs::write(
                        r.trash_path.as_ref().unwrap(),
                        b"OWNED changed trashed content",
                    )
                    .unwrap(),
                    1 => fs::write(root.join("source"), b"OWNED new source entry").unwrap(),
                    2 => {
                        fs::write(r.backup_path.as_ref().unwrap(), b"OWNED changed backup").unwrap()
                    }
                    _ => unreachable!(),
                }
            }
            Ok(())
        });
        assert_eq!(
            value.receipt.status,
            MutationStatus::Incomplete,
            "mode={mode}"
        );
        assert!(value.receipt.mutation_attempted && !value.receipt.completion_verified);
        assert_eq!(value.receipt.moved_to_trash, Some(true));
        if mode != 2 {
            assert_eq!(backup(&value), b"OWNED bytes\0</external>");
        }
        if mode == 1 {
            assert!(!value.receipt.source_absent);
            assert_eq!(
                fs::read(root.join("source")).unwrap(),
                b"OWNED new source entry"
            );
        }
    }
}
#[test]
fn trash_execute_uncertain_submission_retains_original_backup_and_never_reports_completion() {
    let mut value = fixture();
    let mut platform = platform(&value);
    platform.uncertain = true;
    direct(&mut value, &platform, |_| Ok(()));
    assert_eq!(value.receipt.status, MutationStatus::Uncertain);
    assert_eq!(value.receipt.moved_to_trash, None);
    assert!(!value.receipt.completion_verified);
    assert_eq!(
        fs::read(value.root.path().join("source")).unwrap(),
        backup(&value)
    );
    assert!(value.receipt.trash_path.is_none());
}
#[test]
fn trash_execute_refuses_a_trash_symlink_before_backup() {
    let mut value = fixture();
    let original = value.trash.path().to_owned();
    let link = value.root.path().join("trash-link");
    symlink(&original, &link).unwrap();
    value.receipt.request.expected_parent_version =
        crate::files::observe(value.root.path(), Path::new("."), false, None)
            .unwrap()
            .version;
    let mut platform = platform(&value);
    platform.trash = link;
    direct(&mut value, &platform, |_| Ok(()));
    assert_eq!(value.receipt.status, MutationStatus::NotStarted);
    assert!(value.receipt.backup_path.is_none());
    assert_eq!(
        fs::read(value.root.path().join("source")).unwrap(),
        b"OWNED bytes\0</external>"
    );
}
#[test]
fn trash_execute_error_text_is_bounded_at_utf8_boundary_and_false_confirmation_never_allocates() {
    let mut value = fixture();
    let platform = platform(&value);
    direct(&mut value, &platform, |r| {
        if r.stage == TrashStage::BackedUp {
            return Err(FileError::new(FileErrorCode::Io, "臺灣".repeat(10000)));
        }
        Ok(())
    });
    assert!(value.receipt.error_truncated);
    assert!(value.receipt.error.as_ref().unwrap().message.len() <= 1024);
    assert_eq!(value.receipt.status, MutationStatus::NotStarted);
    let mut request = value.receipt.request.clone();
    request.confirm = false;
    assert_eq!(
        run(Path::new("/not/a/worker"), &request, 10000)
            .unwrap_err()
            .code,
        FileErrorCode::InvalidOptions
    );
}
fn xattr(path: &Path, name: &str, value: &str) {
    assert!(
        std::process::Command::new("/usr/bin/xattr")
            .args(["-w", name, value])
            .arg(path)
            .status()
            .unwrap()
            .success()
    );
}
#[test]
fn trash_execute_accepts_native_added_macl_but_keeps_all_original_attributes() {
    for (name, content, completed) in [
        ("com.apple.macl", String::new(), true),
        ("com.apple.macl", "M".repeat(72), true),
        ("com.cueward.owned-unknown", String::new(), false),
    ] {
        let mut value = fixture();
        let platform = platform(&value);
        direct(&mut value, &platform, |r| {
            if r.stage == TrashStage::Verifying {
                xattr(Path::new(r.trash_path.as_ref().unwrap()), name, &content);
            }
            Ok(())
        });
        assert_eq!(value.receipt.completion_verified, completed);
        assert_eq!(
            value.receipt.status,
            if completed {
                MutationStatus::Completed
            } else {
                MutationStatus::Incomplete
            }
        );
        if completed {
            let attributes = value.receipt.trash_attributes.as_ref().unwrap();
            assert!(!attributes.exact_match);
            assert_eq!(
                attributes.accepted_platform_additions,
                [if content.is_empty() {
                    "com.apple.macl (added, zero bytes)".to_owned()
                } else {
                    format!("com.apple.macl (added, {} bytes)", content.len())
                }]
            );
            assert_ne!(
                attributes.sha256,
                value
                    .receipt
                    .backup_verification
                    .as_ref()
                    .unwrap()
                    .extended_attributes_sha256
            );
        }
        assert_eq!(backup(&value), b"OWNED bytes\0</external>");
    }
}
#[test]
fn trash_execute_never_ignores_a_changed_existing_macl_or_other_original_attribute() {
    for name in ["com.apple.macl", "com.cueward.owned-existing"] {
        let mut value = fixture();
        xattr(
            &value.root.path().join("source"),
            name,
            "OWNED original metadata",
        );
        value.receipt.request = request(value.root.path());
        let platform = platform(&value);
        direct(&mut value, &platform, |r| {
            if r.stage == TrashStage::Verifying {
                xattr(Path::new(r.trash_path.as_ref().unwrap()), name, "");
            }
            Ok(())
        });
        assert_eq!(value.receipt.status, MutationStatus::Incomplete);
        assert!(!value.receipt.completion_verified);
        assert_eq!(backup(&value), b"OWNED bytes\0</external>");
    }
}
#[test]
fn trash_execute_native_error_or_missing_result_url_keeps_original_and_backup_at_known_private_or_trash_location()
 {
    for mode in [1, 2] {
        let mut value = fixture();
        let mut platform = platform(&value);
        platform.native_mode = mode;
        direct(&mut value, &platform, |_| Ok(()));
        assert_eq!(value.receipt.source_removed, Some(true));
        assert!(value.receipt.trash_attempted && !value.receipt.completion_verified);
        assert_eq!(
            value.receipt.status,
            if mode == 1 {
                MutationStatus::Uncertain
            } else {
                MutationStatus::Incomplete
            }
        );
        assert!(value.receipt.trash_path.is_none());
        let original = if mode == 1 {
            PathBuf::from(value.receipt.staging_path.as_ref().unwrap())
        } else {
            value.trash.path().join("source")
        };
        assert_eq!(fs::read(original).unwrap(), backup(&value));
        assert!(!value.root.path().join("source").exists());
    }
}
#[test]
fn trash_execute_backup_overlap_and_private_parent_retargeting_never_submit_native_trash() {
    let mut value = fixture();
    let mut platform = platform(&value);
    platform.storage_inside = true;
    fs::remove_file(value.root.path().join("source")).unwrap();
    fs::create_dir(value.root.path().join("source")).unwrap();
    value.receipt.request = request(value.root.path());
    direct(&mut value, &platform, |_| Ok(()));
    assert_eq!(value.receipt.status, MutationStatus::NotStarted);
    assert!(value.receipt.backup_path.is_none());
    assert!(!value.root.path().join("owned-backup").exists());
    assert!(value.root.path().join("source").exists());
    let mut value = fixture();
    let platform = super::tests::platform(&value);
    let outside = tempfile::tempdir().unwrap();
    let moved = outside.path().join("owned-parent");
    direct(&mut value, &platform, |r| {
        if r.stage == TrashStage::Quarantined {
            let parent = Path::new(r.staging_path.as_ref().unwrap())
                .parent()
                .unwrap();
            fs::rename(parent, &moved).unwrap();
            symlink(&moved, parent).unwrap();
        }
        Ok(())
    });
    assert_eq!(value.receipt.status, MutationStatus::Incomplete);
    assert!(!value.receipt.trash_attempted);
    assert!(value.receipt.trash_path.is_none());
    assert_eq!(fs::read(moved.join("source")).unwrap(), backup(&value));
    assert_eq!(fs::read_dir(value.trash.path()).unwrap().count(), 0);
}
