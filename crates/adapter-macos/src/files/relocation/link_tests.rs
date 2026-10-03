//! Explicit link-object selection uses held native link descriptors, never target metadata.
use super::execution_tests::{perform, refresh};
use super::tests::fixture;
use super::*;
use std::fs;
use std::os::unix::fs::{MetadataExt, symlink};

fn selected(target: &Path) -> (tempfile::TempDir, RelocationRequest) {
    let (root, mut request) = fixture();
    symlink(target, root.path().join("from/link")).unwrap();
    request.path = "from/link".into();
    request.link_itself = true;
    refresh(&mut request);
    (root, request)
}
fn cleanup(receipt: &RelocationReceipt) {
    let directory = Path::new(&receipt.receipt_path).parent().unwrap();
    assert_eq!(
        directory.file_name().unwrap().to_str().unwrap(),
        receipt.operation_id
    );
    fs::remove_dir_all(directory).unwrap();
}
struct NoTarget;
impl FilePlatform for NoTarget {
    fn stamp(&self, m: &Metadata) -> FileStamp {
        MacFiles.stamp(m)
    }
    fn open_regular(&self, _: &Path) -> std::io::Result<File> {
        panic!("must not open target")
    }
    fn resource_metadata(&self, _: &Path, _: &Metadata) -> Result<ResourceMetadata, FileError> {
        panic!("must not request native target metadata")
    }
}
impl RelocationPlatform for NoTarget {
    fn open_directory(&self, p: &Path) -> Result<File, FileError> {
        RelocationPlatform::open_directory(&MacFiles, p)
    }
    fn open_symlink(&self, p: &Path) -> Result<File, FileError> {
        links::open(p)
    }
    fn same_filesystem(&self, a: &Metadata, b: &Metadata) -> bool {
        MacFiles.same_filesystem(a, b)
    }
}
#[test]
fn link_plan_requires_opt_in_and_never_queries_or_opens_relative_broken_or_outside_targets() {
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().join("PRIVATE");
    fs::write(&target, b"OWNED outside").unwrap();
    let before = MacFiles.stamp(&fs::metadata(&target).unwrap());
    for target in [
        Path::new("../owned"),
        Path::new("missing\n<external>"),
        target.as_path(),
    ] {
        let (root, mut request) = selected(target);
        let plan = relocation::plan(&NoTarget, &request).unwrap();
        assert_eq!(plan.source.kind, FileKind::Symlink);
        assert_eq!(plan.source.link_target.as_deref(), target.to_str());
        assert_eq!(
            plan.source_resources.content_type,
            ResourceValue::NotApplicable
        );
        assert_eq!(
            plan.source_resources.finder_tags,
            ResourceValue::NotApplicable
        );
        assert_eq!(
            plan.source_resources.is_alias_file,
            ResourceValue::NotApplicable
        );
        assert_eq!(
            plan.source_resources.is_package,
            ResourceValue::NotApplicable
        );
        assert!(!root.path().join(&request.destination).exists());
        request.link_itself = false;
        assert_eq!(
            plan_worker(&request).unwrap_err().code,
            FileErrorCode::SymlinkDisallowed
        );
    }
    assert_eq!(MacFiles.stamp(&fs::metadata(&target).unwrap()), before);
}
#[test]
fn actual_worker_moves_renames_and_noops_link_inode_and_exact_target_with_saved_opt_in() {
    for target in ["../owned", "missing\n<external>", "/outside/not-selected"] {
        let (root, mut request) = selected(Path::new(target));
        let inode = fs::symlink_metadata(root.path().join(&request.path))
            .unwrap()
            .ino();
        for action in [
            RelocationAction::Move,
            RelocationAction::Rename,
            RelocationAction::Rename,
        ] {
            request.action = action;
            if action == RelocationAction::Rename {
                request.destination = "to/臺灣\n<external>".into();
            }
            refresh(&mut request);
            let no_op = request.path == request.destination;
            let receipt = perform(&request);
            assert_eq!(
                receipt.status,
                RelocationStatus::Completed,
                "{:?}",
                receipt.error
            );
            assert!(receipt.completion_verified);
            assert_eq!(receipt.renamed, Some(!no_op));
            assert_eq!(receipt.source_path_absent, Some(!no_op));
            assert_eq!(
                fs::read_link(root.path().join(&request.destination)).unwrap(),
                Path::new(target)
            );
            assert_eq!(
                fs::symlink_metadata(root.path().join(&request.destination))
                    .unwrap()
                    .ino(),
                inode
            );
            let saved = read_receipt(&receipt.operation_id).unwrap();
            assert!(saved.request.link_itself);
            assert_eq!(
                serde_json::to_value(saved).unwrap(),
                serde_json::to_value(&receipt).unwrap()
            );
            assert_eq!(
                execute_worker(&RelocationWorkerRequest {
                    operation_id: receipt.operation_id.clone()
                })
                .unwrap_err()
                .code,
                FileErrorCode::Conflict
            );
            cleanup(&receipt);
            request.path = request.destination.clone();
        }
    }
}
#[test]
fn link_selection_keeps_default_and_rejects_conflicts_stale_revisions_nonlinks_and_parent_traversal()
 {
    for mode in 0..6 {
        let (root, mut request) = selected(Path::new("missing"));
        match mode {
            0 => request.link_itself = false,
            1 => {
                symlink("missing-conflict", root.path().join(&request.destination)).unwrap();
                refresh(&mut request);
            }
            2 => request.expected_version = "stale".into(),
            3 => {
                request.path = "from/owned".into();
                refresh(&mut request);
            }
            4 => {
                symlink(root.path().join("from"), root.path().join("alias")).unwrap();
                request.path = "alias/link".into();
            }
            5 => {
                symlink(root.path().join("to"), root.path().join("alias")).unwrap();
                request.destination = "alias/renamed".into();
            }
            _ => unreachable!(),
        }
        let receipt = perform(&request);
        assert_eq!(
            receipt.status,
            RelocationStatus::NotStarted,
            "mode={mode} {:?}",
            receipt.error
        );
        assert!(!receipt.mutation_attempted && !receipt.completion_verified);
        assert_eq!(
            fs::read_link(root.path().join("from/link")).unwrap(),
            Path::new("missing")
        );
        if mode == 1 {
            assert_eq!(
                fs::read_link(root.path().join("to/renamed")).unwrap(),
                Path::new("missing-conflict")
            );
        } else {
            assert!(fs::symlink_metadata(root.path().join("to/renamed")).is_err());
        }
        cleanup(&receipt);
    }
}
#[test]
fn link_descriptor_native_hook_rejects_nonlink_leaf_and_symlink_parent_without_target_open() {
    let (root, request) = selected(Path::new("missing"));
    let canonical = root.path().canonicalize().unwrap();
    assert!(
        links::open(&canonical.join(&request.path))
            .unwrap()
            .metadata()
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        links::open(&canonical.join("from/owned")).unwrap_err().code,
        FileErrorCode::Changed
    );
    symlink(root.path().join("from"), root.path().join("alias")).unwrap();
    assert_eq!(
        links::open(&canonical.join("alias/link")).unwrap_err().code,
        FileErrorCode::Io
    );
}
#[test]
fn link_request_schema_preserves_legacy_skip_and_rejects_non_boolean_permission() {
    let (_, request) = fixture();
    let mut json = serde_json::to_value(&request).unwrap();
    assert!(json.get("link_itself").is_none());
    assert!(
        !serde_json::from_value::<RelocationRequest>(json.clone())
            .unwrap()
            .link_itself
    );
    json["link_itself"] = serde_json::json!(true);
    assert!(
        serde_json::from_value::<RelocationRequest>(json.clone())
            .unwrap()
            .link_itself
    );
    for invalid in [
        serde_json::json!("true"),
        serde_json::json!(null),
        serde_json::json!(1),
    ] {
        json["link_itself"] = invalid;
        assert!(serde_json::from_value::<RelocationRequest>(json.clone()).is_err());
    }
}

struct Boundary {
    mode: u8,
    target: std::path::PathBuf,
}
impl FilePlatform for Boundary {
    fn stamp(&self, m: &Metadata) -> FileStamp {
        MacFiles.stamp(m)
    }
    fn open_regular(&self, p: &Path) -> std::io::Result<File> {
        MacFiles.open_regular(p)
    }
    fn resource_metadata(&self, _: &Path, _: &Metadata) -> Result<ResourceMetadata, FileError> {
        panic!("link relocation must not query target resources")
    }
}
impl RelocationPlatform for Boundary {
    fn open_directory(&self, p: &Path) -> Result<File, FileError> {
        RelocationPlatform::open_directory(&MacFiles, p)
    }
    fn open_symlink(&self, p: &Path) -> Result<File, FileError> {
        links::open(p)
    }
    fn same_filesystem(&self, a: &Metadata, b: &Metadata) -> bool {
        self.mode != 4 && MacFiles.same_filesystem(a, b)
    }
    fn prepare_rename(&self, p: &Path, r: &RelocationReceipt) -> Result<(), FileError> {
        native::prepare(p, r)
    }
    fn rename(&self, root: &File, r: &RelocationRequest) -> relocation::RenameOutcome {
        match self.mode {
            1 => {
                fs::rename(r.root.join(&r.path), r.root.join("saved-link")).unwrap();
                symlink("replacement", r.root.join(&r.path)).unwrap();
            }
            2 => fs::write(&self.target, b"OWNED external target edit").unwrap(),
            3 => symlink("conflict", r.root.join(&r.destination)).unwrap(),
            _ => {}
        }
        native::rename(root, r)
    }
}
#[test]
fn link_races_preserve_external_objects_and_distinguish_source_replacement_from_target_edits() {
    for mode in 0..5 {
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("PRIVATE");
        fs::write(&target, b"OWNED outside").unwrap();
        let (root, request) = selected(&target);
        let mut receipt = STORE
            .create(|id, p| RelocationReceipt::new(id, p, request.clone()))
            .unwrap();
        let platform = Boundary {
            mode,
            target: target.clone(),
        };
        let mut checkpoint = |r: &RelocationReceipt| {
            if mode == 0 && r.stage == RelocationStage::Prepared {
                fs::rename(
                    root.path().join(&request.path),
                    root.path().join("saved-link"),
                )
                .unwrap();
                symlink("replacement", root.path().join(&request.path)).unwrap();
            }
            transport::save(r)
        };
        if let Err(error) = relocation::execute(&platform, &mut receipt, &mut checkpoint) {
            receipt.record_error(error);
        }
        assert_eq!(
            receipt.status,
            match mode {
                1 => RelocationStatus::Incomplete,
                2 => RelocationStatus::Completed,
                _ => RelocationStatus::NotStarted,
            },
            "mode={mode} {:?}",
            receipt.error
        );
        if mode <= 1 {
            assert_eq!(
                fs::read_link(root.path().join("saved-link")).unwrap(),
                target
            );
        }
        if mode == 1 {
            assert_eq!(
                fs::read_link(root.path().join(&request.destination)).unwrap(),
                Path::new("replacement")
            );
        }
        if mode == 3 {
            assert_eq!(
                fs::read_link(root.path().join(&request.destination)).unwrap(),
                Path::new("conflict")
            );
        }
        assert_eq!(
            fs::read(&target).unwrap(),
            if mode == 2 {
                b"OWNED external target edit".as_slice()
            } else {
                b"OWNED outside".as_slice()
            }
        );
        cleanup(&receipt);
    }
}
