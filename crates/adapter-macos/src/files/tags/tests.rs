use super::super::MacFiles;
use super::*;
use cueward_core::files::tags::TagEdit;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};

pub(super) fn fixture() -> (tempfile::TempDir, TagsRequest) {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("owned.txt"), b"OWNED\0tag fixture").unwrap();
    let original = read(root.path(), Path::new("owned.txt"), None).unwrap();
    let request = TagsRequest {
        root: root.path().to_owned(),
        path: "owned.txt".into(),
        expected_version: original.file.version,
        expected_tags_version: original.tags_version,
        edit: TagEdit::Add(vec!["新增 <external> 臺灣".into()]),
    };
    (root, request)
}
pub(super) fn direct(
    request: &TagsRequest,
    checkpoint: &mut impl FnMut(&TagsReceipt) -> Result<(), FileError>,
) -> TagsReceipt {
    let mut receipt = STORE
        .create(|id, path| TagsReceipt::new(id, path, request.clone()))
        .unwrap();
    let result = operation::execute(&mut receipt, &mut |r| {
        save(r)?;
        checkpoint(r)
    });
    if let Err(error) = result {
        record_error(&mut receipt, error);
    }
    save(&receipt).unwrap();
    let loaded = read_receipt(&receipt.operation_id).unwrap();
    fs::remove_dir_all(STORE.directory(&receipt.operation_id).unwrap()).unwrap();
    loaded
}
pub(super) fn refresh(request: &mut TagsRequest) {
    let value = read(&request.root, &request.path, None).unwrap();
    request.expected_version = value.file.version;
    request.expected_tags_version = value.tags_version;
}
#[test]
fn read_and_noop_preserve_colors_payload_inode_permissions_and_mtime() {
    let (root, mut request) = fixture();
    let path = root.path().join(&request.path);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    let file = MacFiles
        .open_regular(&path.canonicalize().unwrap())
        .unwrap();
    let initial = vec![
        FileTag {
            name: "Keep".into(),
            color: Some(6),
        },
        FileTag {
            name: "Other".into(),
            color: None,
        },
    ];
    native::write(&file, &codec::encode(&initial).unwrap(), false).unwrap();
    request.edit = TagEdit::Add(vec!["Keep".into()]);
    refresh(&mut request);
    let before = file.metadata().unwrap();
    let payload = fs::read(&path).unwrap();
    let receipt = direct(&request, &mut |_| Ok(()));
    assert_eq!(receipt.status, TagsStatus::Completed, "{:?}", receipt.error);
    assert!(receipt.completion_verified && receipt.original_attribute_base64.is_some());
    assert!(!receipt.mutation_attempted);
    assert_eq!(receipt.after.unwrap().tags, initial);
    request.edit = TagEdit::Remove(vec!["Other".into()]);
    refresh(&mut request);
    assert_eq!(
        direct(&request, &mut |_| Ok(())).status,
        TagsStatus::NotStarted
    );
    assert_eq!(
        read(root.path(), &request.path, None).unwrap().tags,
        initial
    );
    let after = file.metadata().unwrap();
    assert_eq!(fs::read(path).unwrap(), payload);
    assert_eq!(
        (after.ino(), after.mode(), after.mtime(), after.mtime_nsec()),
        (
            before.ino(),
            before.mode(),
            before.mtime(),
            before.mtime_nsec()
        )
    );
}
#[test]
fn repeated_add_and_missing_remove_are_verified_noops() {
    let (_root, mut request) = fixture();
    let added = direct(&request, &mut |_| Ok(()));
    assert_eq!(added.status, TagsStatus::Completed);
    refresh(&mut request);
    let receipt = direct(&request, &mut |_| Ok(()));
    assert_eq!(receipt.status, TagsStatus::Completed);
    assert_eq!(receipt.changed_by_operation, Some(false));
    assert!(!receipt.mutation_attempted);
    request.edit = TagEdit::Remove(vec!["missing".into()]);
    assert_eq!(
        direct(&request, &mut |_| Ok(())).changed_by_operation,
        Some(false)
    );
}
#[test]
fn stale_file_or_tag_revision_never_submits() {
    let (_root, mut request) = fixture();
    request.expected_tags_version = "stale".into();
    let receipt = direct(&request, &mut |_| Ok(()));
    assert_eq!(receipt.status, TagsStatus::NotStarted);
    assert!(!receipt.mutation_attempted);
    assert!(
        read(&request.root, &request.path, None)
            .unwrap()
            .tags
            .is_empty()
    );
    refresh(&mut request);
    request.expected_version = "stale".into();
    assert_eq!(
        direct(&request, &mut |_| Ok(())).status,
        TagsStatus::NotStarted
    );
}
#[test]
fn prepared_checkpoint_failure_preserves_original_tags_and_saved_backup() {
    let (_root, request) = fixture();
    let receipt = direct(&request, &mut |r| {
        if r.stage == TagsStage::Prepared {
            Err(FileError::new(
                FileErrorCode::Io,
                "owned checkpoint failure",
            ))
        } else {
            Ok(())
        }
    });
    assert_eq!(receipt.status, TagsStatus::NotStarted);
    assert!(receipt.before.is_some());
    assert!(
        read(&request.root, &request.path, None)
            .unwrap()
            .tags
            .is_empty()
    );
}
#[test]
fn writing_checkpoint_external_tag_change_is_not_overwritten() {
    let (_root, request) = fixture();
    let file = MacFiles
        .open_regular(&request.root.join(&request.path).canonicalize().unwrap())
        .unwrap();
    let external = vec![FileTag {
        name: "External".into(),
        color: Some(2),
    }];
    let receipt = direct(&request, &mut |r| {
        if r.stage == TagsStage::Writing {
            native::write(&file, &codec::encode(&external)?, false)?;
        }
        Ok(())
    });
    assert_eq!(receipt.status, TagsStatus::NotStarted);
    assert_eq!(receipt.changed_by_operation, Some(false));
    assert_eq!(native::read(&file).unwrap().tags, external);
}
#[test]
fn path_replaced_at_writing_checkpoint_never_tags_the_replacement() {
    let (root, request) = fixture();
    let receipt = direct(&request, &mut |r| {
        if r.stage == TagsStage::Writing {
            fs::rename(root.path().join("owned.txt"), root.path().join("moved.txt"))?;
            fs::write(root.path().join("owned.txt"), b"EXTERNAL replacement")?;
        }
        Ok(())
    });
    assert_eq!(receipt.status, TagsStatus::NotStarted);
    assert!(
        read(root.path(), &request.path, None)
            .unwrap()
            .tags
            .is_empty()
    );
    assert!(
        read(root.path(), Path::new("moved.txt"), None)
            .unwrap()
            .tags
            .is_empty()
    );
}
#[test]
fn checkpoint_failure_after_write_is_incomplete_without_rollback() {
    let (_root, request) = fixture();
    let receipt = direct(&request, &mut |r| {
        if r.changed_by_operation == Some(true) {
            Err(FileError::new(
                FileErrorCode::Io,
                "owned post-write failure",
            ))
        } else {
            Ok(())
        }
    });
    assert_eq!(receipt.status, TagsStatus::Incomplete);
    assert!(!receipt.completion_verified);
    assert_eq!(
        read(&request.root, &request.path, None).unwrap().tags.len(),
        1
    );
}
#[test]
fn directories_packages_are_supported_and_hardlinked_edits_are_rejected() {
    let (root, mut request) = fixture();
    fs::create_dir(root.path().join("Owned.app")).unwrap();
    request.path = "Owned.app".into();
    refresh(&mut request);
    assert_eq!(
        direct(&request, &mut |_| Ok(())).status,
        TagsStatus::Completed
    );
    request.path = "owned.txt".into();
    refresh(&mut request);
    fs::hard_link(root.path().join("owned.txt"), root.path().join("link.txt")).unwrap();
    refresh(&mut request);
    let rejected = direct(&request, &mut |_| Ok(()));
    assert_eq!(rejected.status, TagsStatus::NotStarted);
    assert_eq!(rejected.error.unwrap().code, FileErrorCode::UnsupportedType);
    assert!(
        read(root.path(), Path::new("link.txt"), None)
            .unwrap()
            .tags
            .is_empty()
    );
}
#[test]
fn links_outside_paths_and_invalid_attribute_formats_are_rejected() {
    let (root, request) = fixture();
    symlink(root.path().join("owned.txt"), root.path().join("symlink")).unwrap();
    assert!(read(root.path(), Path::new("symlink"), None).is_err());
    assert!(read(root.path(), Path::new("../outside"), None).is_err());
    assert!(read(root.path(), Path::new("/absolute"), None).is_err());
    let file = MacFiles
        .open_regular(&root.path().join(&request.path).canonicalize().unwrap())
        .unwrap();
    native::write(&file, b"not a property list", false).unwrap();
    assert_eq!(
        read(root.path(), &request.path, None).unwrap_err().code,
        FileErrorCode::UnsupportedType
    );
    assert_eq!(
        direct(&request, &mut |_| Ok(())).status,
        TagsStatus::NotStarted
    );
}
#[test]
fn private_receipts_claim_once_and_reject_path_traversal() {
    let (_root, request) = fixture();
    let prepared = STORE
        .create(|id, path| TagsReceipt::new(id, path, request))
        .unwrap();
    let input = TagsWorkerRequest {
        operation_id: prepared.operation_id.clone(),
    };
    assert_eq!(
        execute_worker(&input).unwrap().status,
        TagsStatus::Completed
    );
    assert_eq!(
        execute_worker(&input).unwrap_err().code,
        FileErrorCode::Conflict
    );
    assert_eq!(
        fs::metadata(&prepared.receipt_path).unwrap().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(STORE.directory(&input.operation_id).unwrap())
            .unwrap()
            .mode()
            & 0o777,
        0o700
    );
    assert!(read_receipt("../outside").is_err());
    fs::remove_dir_all(STORE.directory(&input.operation_id).unwrap()).unwrap();
}
#[test]
fn codec_preserves_absence_colors_and_rejects_unsupported_shapes() {
    assert_ne!(
        codec::Stored::decode(None).unwrap().version,
        codec::Stored::decode(Some(codec::encode(&[]).unwrap()))
            .unwrap()
            .version
    );
    let tags = vec![
        FileTag {
            name: "Color".into(),
            color: Some(7),
        },
        FileTag {
            name: "Plain".into(),
            color: None,
        },
    ];
    assert_eq!(
        codec::Stored::decode(Some(codec::encode(&tags).unwrap()))
            .unwrap()
            .tags,
        tags
    );
    for value in [
        plist::Value::Array(vec![plist::Value::Integer(1.into())]),
        plist::Value::String("unexpected".into()),
        plist::Value::Array(vec![plist::Value::String("Name\n8".into())]),
    ] {
        let mut bytes = Vec::new();
        value.to_writer_binary(&mut bytes).unwrap();
        assert!(codec::Stored::decode(Some(bytes)).is_err());
    }
}

#[test]
fn unrelated_xattrs_finder_info_and_existing_color_entries_are_unchanged() {
    let (root, request) = fixture();
    let path = root.path().join(&request.path);
    let _file = prepare_color_fixture(&path);
    let mut request = request;
    refresh(&mut request);
    let attrs = |name| {
        std::process::Command::new("/usr/bin/xattr")
            .args(["-px", name])
            .arg(&path)
            .output()
            .unwrap()
            .stdout
    };
    let finder = attrs("com.apple.FinderInfo");
    let unrelated = attrs("com.cueward.owned");
    assert_eq!(
        direct(&request, &mut |_| Ok(())).status,
        TagsStatus::NotStarted
    );
    assert_eq!(attrs("com.apple.FinderInfo"), finder);
    assert_eq!(attrs("com.cueward.owned"), unrelated);
    request.edit = TagEdit::Remove(vec!["Existing".into(), "新增 <external> 臺灣".into()]);
    refresh(&mut request);
    let rejected = direct(&request, &mut |_| Ok(()));
    assert_eq!(rejected.status, TagsStatus::NotStarted);
    assert_eq!(rejected.error.unwrap().code, FileErrorCode::UnsupportedType);
    assert_eq!(
        read(root.path(), &request.path, None).unwrap().tags.len(),
        1
    );
}

#[test]
fn relocation_after_submission_is_incomplete_and_preserves_the_replacement() {
    let (root, request) = fixture();
    let receipt = direct(&request, &mut |r| {
        if r.stage == TagsStage::Verifying {
            fs::rename(root.path().join("owned.txt"), root.path().join("moved.txt"))?;
            fs::write(root.path().join("owned.txt"), b"EXTERNAL replacement")?;
        }
        Ok(())
    });
    assert_eq!(receipt.status, TagsStatus::Incomplete);
    assert!(!receipt.completion_verified);
    assert!(
        read(root.path(), &request.path, None)
            .unwrap()
            .tags
            .is_empty()
    );
    assert_eq!(
        read(root.path(), Path::new("moved.txt"), None)
            .unwrap()
            .tags
            .len(),
        1
    );
    assert_eq!(
        fs::read(root.path().join("owned.txt")).unwrap(),
        b"EXTERNAL replacement"
    );
}
#[test]
fn native_permission_denial_on_an_owned_immutable_file_does_not_change_tags() {
    struct Reset(std::path::PathBuf);
    impl Drop for Reset {
        fn drop(&mut self) {
            let _ = std::process::Command::new("/usr/bin/chflags")
                .arg("nouchg")
                .arg(&self.0)
                .status();
        }
    }
    let (root, mut request) = fixture();
    let path = root.path().join("owned.txt");
    let _reset = Reset(path.clone());
    assert!(
        std::process::Command::new("/usr/bin/chflags")
            .arg("uchg")
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    refresh(&mut request);
    let receipt = direct(&request, &mut |_| Ok(()));
    assert_eq!(receipt.status, TagsStatus::NotStarted);
    assert_eq!(receipt.error.unwrap().code, FileErrorCode::PermissionDenied);
    assert_eq!(receipt.changed_by_operation, Some(false));
    assert!(
        read(root.path(), &request.path, None)
            .unwrap()
            .tags
            .is_empty()
    );
}

fn prepare_color_fixture(path: &Path) -> File {
    for (name, data) in [
        ("com.cueward.owned", "0010aaff"),
        (
            "com.apple.FinderInfo",
            "0000000000000000000c00000000000000000000000000000000000000000000",
        ),
    ] {
        assert!(
            std::process::Command::new("/usr/bin/xattr")
                .args(["-wx", name, data])
                .arg(path)
                .status()
                .unwrap()
                .success()
        );
    }
    let file = MacFiles
        .open_regular(&path.canonicalize().unwrap())
        .unwrap();
    native::write(
        &file,
        &codec::encode(&[FileTag {
            name: "Existing".into(),
            color: Some(6),
        }])
        .unwrap(),
        false,
    )
    .unwrap();
    file
}
