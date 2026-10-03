use super::*;
use cueward_core::files::copy_tree::CopyTreePlatform;
use cueward_core::files::mutation::MutationStatus;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};

pub(super) struct Owned {
    pub root: tempfile::TempDir,
    pub receipt: TreeReceipt,
}
impl Drop for Owned {
    fn drop(&mut self) {
        let directory = Path::new(&self.receipt.receipt_path).parent().unwrap();
        assert_eq!(
            directory.file_name().unwrap().to_str().unwrap(),
            self.receipt.operation_id
        );
        fs::remove_dir_all(directory).unwrap();
    }
}
pub(super) fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("source/sub/empty")).unwrap();
    fs::write(root.path().join("source/.hidden"), b"OWNED hidden\0").unwrap();
    fs::write(
        root.path().join("source/sub/臺灣\n<external>"),
        b"OWNED nested",
    )
    .unwrap();
    root
}
pub(super) fn request(root: &Path) -> CopyTreeRequest {
    let observed = |path| {
        crate::files::observe(root, Path::new(path), false, None)
            .unwrap()
            .version
    };
    CopyTreeRequest {
        root: root.into(),
        path: "source".into(),
        destination: "destination".into(),
        expected_version: observed("source"),
        expected_parent_version: observed("."),
        max_entries: 64,
        max_depth: 16,
        max_bytes: 67108864,
        include_packages: false,
    }
}
pub(super) fn owned(root: tempfile::TempDir, request: CopyTreeRequest) -> Owned {
    let receipt = STORE
        .create(|id, path| TreeReceipt::new(id, path, request))
        .unwrap();
    Owned { root, receipt }
}
pub(super) fn perform(value: &mut Owned) {
    value.receipt = execute_worker(&TreeWorkerRequest {
        operation_id: value.receipt.operation_id.clone(),
    })
    .unwrap();
}
pub(super) fn direct(
    platform: &impl TreeExecutionPlatform,
    value: &mut Owned,
    mut checkpoint: impl FnMut(&TreeReceipt) -> Result<(), FileError>,
) {
    let result = execution::execute(platform, &mut value.receipt, &mut |r| {
        checkpoint(r)?;
        save(r)
    });
    if let Err(error) = result {
        execution::record_error(&mut value.receipt, error);
    }
    save(&value.receipt).unwrap();
}
fn attr(path: &Path, name: &str, value: &str) {
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
fn tree_execute_verified_nested_hidden_empty_xattrs_and_independent_hardlinks() {
    let root = fixture();
    fs::write(root.path().join("source/empty-file"), b"").unwrap();
    fs::hard_link(
        root.path().join("source/.hidden"),
        root.path().join("source/hardlink"),
    )
    .unwrap();
    attr(
        &root.path().join("source"),
        "com.cueward.owned-tree",
        "OWNED directory metadata",
    );
    attr(
        &root.path().join("source/.hidden"),
        "com.cueward.owned-tree",
        "OWNED file metadata",
    );
    fs::set_permissions(
        root.path().join("source"),
        fs::Permissions::from_mode(0o750),
    )
    .unwrap();
    let before = MacFiles.stamp(&fs::metadata(root.path().join("source/.hidden")).unwrap());
    let selected = request(root.path());
    let mut value = owned(root, selected);
    perform(&mut value);
    assert_eq!(
        value.receipt.status,
        MutationStatus::Completed,
        "{:?}",
        value.receipt.error
    );
    assert!(value.receipt.staging_verified && value.receipt.completion_verified);
    assert_eq!(value.receipt.destination_created, Some(true));
    assert_eq!(value.receipt.nodes.len(), 7);
    assert!(!Path::new(value.receipt.staging_path.as_ref().unwrap()).exists());
    for node in &value.receipt.nodes {
        let relative = &node.relative_path;
        let source = value.root.path().join("source").join(relative);
        let target = value.root.path().join("destination").join(relative);
        assert!(target.exists());
        assert_ne!(
            fs::metadata(&source).unwrap().ino(),
            fs::metadata(&target).unwrap().ino()
        );
        assert_eq!(
            fs::metadata(&source).unwrap().permissions(),
            fs::metadata(&target).unwrap().permissions()
        );
        assert_eq!(
            fs::metadata(&source).unwrap().modified().unwrap(),
            fs::metadata(&target).unwrap().modified().unwrap()
        );
        if node.kind == FileKind::File {
            assert_eq!(fs::read(&source).unwrap(), fs::read(&target).unwrap());
        }
        let open = |p: &Path| {
            if p.is_dir() {
                MacFiles
                    .open_tree_directory(&p.canonicalize().unwrap())
                    .unwrap()
            } else {
                MacFiles.open_regular(&p.canonicalize().unwrap()).unwrap()
            }
        };
        assert_eq!(
            MacFiles.tree_attribute_digest(&open(&source)).unwrap(),
            MacFiles.tree_attribute_digest(&open(&target)).unwrap()
        );
    }
    assert_eq!(
        MacFiles.stamp(&fs::metadata(value.root.path().join("source/.hidden")).unwrap()),
        before
    );
    assert_ne!(
        fs::metadata(value.root.path().join("destination/.hidden"))
            .unwrap()
            .ino(),
        fs::metadata(value.root.path().join("destination/hardlink"))
            .unwrap()
            .ino()
    );
    let loaded = read_receipt(&value.receipt.operation_id).unwrap();
    assert_eq!(json(&loaded).unwrap(), json(&value.receipt).unwrap());
    assert!(crate::files::mutation::read_receipt(&value.receipt.operation_id).is_err());
    assert!(
        execute_worker(&TreeWorkerRequest {
            operation_id: value.receipt.operation_id.clone()
        })
        .is_err()
    );
}
#[test]
fn tree_execute_preflight_refuses_conflicts_links_packages_and_limits_without_staging() {
    for mode in 0..6 {
        let root = fixture();
        match mode {
            0 => fs::write(root.path().join("destination"), b"OWNED conflict").unwrap(),
            1 => symlink("missing", root.path().join("destination")).unwrap(),
            2 => symlink("missing", root.path().join("source/link")).unwrap(),
            3 => fs::create_dir(root.path().join("source/Owned.app")).unwrap(),
            _ => (),
        }
        let mut selected = request(root.path());
        if mode == 4 {
            selected.max_entries = 1;
        }
        if mode == 5 {
            selected.expected_version = "stale".into();
        }
        let mut value = owned(root, selected);
        perform(&mut value);
        assert_eq!(value.receipt.status, MutationStatus::NotStarted);
        assert!(
            !value.receipt.mutation_attempted
                && !value.receipt.staging_verified
                && !value.receipt.completion_verified
        );
        assert!(value.receipt.staging_path.is_none());
        assert!(value.receipt.error.is_some());
        assert_eq!(
            fs::read(value.root.path().join("source/.hidden")).unwrap(),
            b"OWNED hidden\0"
        );
    }
}
#[test]
fn tree_execute_empty_tree_succeeds_with_one_entry_and_no_file_hash() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("source")).unwrap();
    let mut selected = request(root.path());
    selected.max_entries = 1;
    let mut value = owned(root, selected);
    perform(&mut value);
    assert_eq!(
        value.receipt.status,
        MutationStatus::Completed,
        "{:?}",
        value.receipt.error
    );
    assert!(value.receipt.nodes[0].verification.is_none());
    assert_eq!(
        fs::read_dir(value.root.path().join("destination"))
            .unwrap()
            .count(),
        0
    );
}
#[test]
fn tree_execute_source_edits_after_partial_copy_preserve_staging_without_publication() {
    let root = fixture();
    let selected = request(root.path());
    let mut value = owned(root, selected);
    let path = value.root.path().join("source/.hidden");
    let mut changed = false;
    direct(&MacFiles, &mut value, |r| {
        if r.stage == TreeStage::Copying
            && r.nodes.iter().any(|n| n.verification.is_some())
            && !changed
        {
            changed = true;
            fs::write(&path, b"OWNED external change").unwrap();
        }
        Ok(())
    });
    assert!(changed);
    assert_eq!(value.receipt.status, MutationStatus::NotStarted);
    assert!(!value.root.path().join("destination").exists());
    let staging = Path::new(value.receipt.staging_path.as_ref().unwrap());
    assert!(staging.join(".hidden").exists());
    assert_eq!(
        fs::read(staging.join(".hidden")).unwrap(),
        b"OWNED hidden\0"
    );
    assert_eq!(fs::read(path).unwrap(), b"OWNED external change");
}
#[test]
fn tree_execute_checkpoint_failure_and_late_conflict_keep_private_evidence() {
    for late_conflict in [false, true] {
        let root = fixture();
        let selected = request(root.path());
        let mut value = owned(root, selected);
        let path = value.root.path().join("destination");
        direct(&MacFiles, &mut value, |r| {
            if r.stage == TreeStage::Staged {
                if late_conflict {
                    fs::write(&path, b"OWNED conflict").unwrap();
                } else {
                    return Err(FileError::new(
                        FileErrorCode::Io,
                        "owned injected checkpoint failure",
                    ));
                }
            }
            Ok(())
        });
        assert_eq!(value.receipt.status, MutationStatus::NotStarted);
        assert!(value.receipt.staging_verified);
        assert!(Path::new(value.receipt.staging_path.as_ref().unwrap()).is_dir());
        if late_conflict {
            assert_eq!(fs::read(path).unwrap(), b"OWNED conflict");
        } else {
            assert!(!path.exists());
        }
    }
}
#[test]
fn tree_execute_changes_after_publication_are_incomplete_without_repair_or_cleanup() {
    let root = fixture();
    let selected = request(root.path());
    let mut value = owned(root, selected);
    let path = value.root.path().join("destination/.hidden");
    direct(&MacFiles, &mut value, |r| {
        if r.stage == TreeStage::Verifying {
            fs::write(&path, b"OWNED external change").unwrap();
        }
        Ok(())
    });
    assert_eq!(value.receipt.status, MutationStatus::Incomplete);
    assert_eq!(value.receipt.destination_created, Some(true));
    assert!(!value.receipt.completion_verified);
    assert_eq!(fs::read(path).unwrap(), b"OWNED external change");
    assert_eq!(
        fs::read(value.root.path().join("source/.hidden")).unwrap(),
        b"OWNED hidden\0"
    );
}
