use super::*;
use cueward_core::files::relocation::batch::BatchRenameEntry;
use std::fs;
use std::os::unix::fs::{MetadataExt, symlink};

fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    for name in ["a", "b"] {
        fs::write(root.path().join(name), format!("OWNED {name}\0")).unwrap();
    }
    root
}
fn request(root: &Path, entries: &[(&str, &str)]) -> BatchRenameRequest {
    BatchRenameRequest {
        root: root.into(),
        entries: entries
            .iter()
            .map(|(path, name)| {
                let parent = Path::new(path)
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or(Path::new("."));
                BatchRenameEntry {
                    path: path.into(),
                    name: (*name).into(),
                    expected_version: super::super::observe(root, Path::new(path), false, None)
                        .unwrap()
                        .version,
                    expected_parent_version: super::super::observe(root, parent, false, None)
                        .unwrap()
                        .version,
                    link_itself: false,
                }
            })
            .collect(),
    }
}
fn prepare(request: &BatchRenameRequest) -> BatchExecutionReceipt {
    STORE
        .create(|id, path| BatchExecutionReceipt::new(id, path, request.clone()))
        .unwrap()
}
fn perform(
    platform: &impl BatchRenamePlatform,
    request: &BatchRenameRequest,
) -> BatchExecutionReceipt {
    let prepared = prepare(request);
    execute_prepared(
        platform,
        &BatchExecutionWorkerRequest {
            operation_id: prepared.operation_id,
        },
    )
    .unwrap()
}
fn cleanup(receipt: &BatchExecutionReceipt) {
    for item in &receipt.items {
        if let Some(path) = &item.receipt_path {
            let directory = Path::new(path).parent().unwrap();
            assert_eq!(
                directory.file_name().unwrap().to_str(),
                item.operation_id.as_deref()
            );
            fs::remove_dir_all(directory).unwrap();
        }
    }
    let directory = Path::new(&receipt.receipt_path).parent().unwrap();
    assert_eq!(
        directory.file_name().unwrap().to_str(),
        Some(receipt.operation_id.as_str())
    );
    fs::remove_dir_all(directory).unwrap();
}
#[test]
fn batch_execute_advances_sibling_parent_guards_and_preserves_original_request() {
    let root = fixture();
    let aliases = tempfile::tempdir().unwrap();
    let alias = aliases.path().join("root");
    symlink(root.path(), &alias).unwrap();
    let request = request(&alias, &[("a", "臺灣\n<external>"), ("b", "other")]);
    let inode = root.path().join("a").metadata().unwrap().ino();
    let receipt = perform(&MacFiles, &request);
    assert_eq!(
        receipt.status,
        RelocationStatus::Completed,
        "{:?}",
        receipt.error
    );
    assert!(receipt.completion_verified && receipt.active_index.is_none());
    assert_eq!(receipt.request.root, alias);
    assert_eq!(
        receipt.request.entries[1].expected_parent_version,
        request.entries[1].expected_parent_version
    );
    let child = relocation::read_receipt(receipt.items[1].operation_id.as_ref().unwrap()).unwrap();
    assert_ne!(
        child.request.expected_parent_version,
        request.entries[1].expected_parent_version
    );
    assert_eq!(child.request.root, root.path().canonicalize().unwrap());
    assert_eq!(
        root.path()
            .join("臺灣\n<external>")
            .metadata()
            .unwrap()
            .ino(),
        inode
    );
    assert_eq!(fs::read(root.path().join("other")).unwrap(), b"OWNED b\0");
    assert!(!root.path().join("a").exists() && !root.path().join("b").exists());
    assert_eq!(
        serde_json::to_value(read_receipt(&receipt.operation_id).unwrap()).unwrap(),
        serde_json::to_value(&receipt).unwrap()
    );
    assert!(
        execute_worker(&BatchExecutionWorkerRequest {
            operation_id: receipt.operation_id.clone()
        })
        .is_err()
    );
    cleanup(&receipt);
}
#[test]
fn batch_execute_handles_separate_parents_directories_packages_and_noops() {
    let root = fixture();
    fs::create_dir(root.path().join("nested")).unwrap();
    fs::create_dir(root.path().join("nested/Owned.app")).unwrap();
    fs::write(
        root.path().join("nested/Owned.app/payload"),
        b"OWNED package",
    )
    .unwrap();
    let r = perform(
        &MacFiles,
        &request(
            root.path(),
            &[
                ("a", "a"),
                ("nested/Owned.app", "Renamed.app"),
                ("b", "new"),
            ],
        ),
    );
    assert_eq!(r.status, RelocationStatus::Completed, "{:?}", r.error);
    assert_eq!(r.items[0].renamed, Some(false));
    assert!(!r.items[0].mutation_attempted && r.items[0].completion_verified);
    assert_eq!(
        fs::read(root.path().join("nested/Renamed.app/payload")).unwrap(),
        b"OWNED package"
    );
    cleanup(&r);
}
#[test]
fn batch_execute_conflicts_errors_and_dependencies_refuse_all_writes() {
    let root = fixture();
    fs::hard_link(root.path().join("a"), root.path().join("link")).unwrap();
    fs::create_dir(root.path().join("dir")).unwrap();
    fs::write(root.path().join("dir/child"), b"OWNED child").unwrap();
    for entries in [
        vec![("a", "b"), ("b", "a")],
        vec![("a", "same"), ("b", "same")],
        vec![("a", "Report"), ("b", "report")],
        vec![("a", "new"), ("link", "other")],
        vec![("dir", "newdir"), ("dir/child", "new")],
        vec![("a", "new"), ("b", "../invalid")],
    ] {
        let r = perform(&MacFiles, &request(root.path(), &entries));
        assert_eq!(r.status, RelocationStatus::NotStarted);
        assert!(
            r.items
                .iter()
                .all(|i| i.operation_id.is_none() && !i.mutation_attempted)
        );
        assert_eq!(fs::read(root.path().join("a")).unwrap(), b"OWNED a\0");
        assert_eq!(fs::read(root.path().join("b")).unwrap(), b"OWNED b\0");
        cleanup(&r);
    }
    let mut stale = request(root.path(), &[("a", "new"), ("b", "other")]);
    stale.entries[1].expected_version = "stale".into();
    let r = perform(&MacFiles, &stale);
    assert_eq!(
        r.items[1].error.as_ref().unwrap().code,
        FileErrorCode::Changed
    );
    assert_eq!(r.status, RelocationStatus::NotStarted);
    cleanup(&r);
}

#[path = "batch_execution_race_tests.rs"]
mod races;

#[test]
fn batch_execute_rejects_malformed_or_progressed_records_before_any_child() {
    for change in [0, 1, 2] {
        let root = fixture();
        let mut receipt = prepare(&request(root.path(), &[("a", "new")]));
        match change {
            0 => receipt.items.clear(),
            1 => receipt.items[0].index = 9,
            _ => receipt.started = true,
        }
        save(&receipt).unwrap();
        let result = execute_worker(&BatchExecutionWorkerRequest {
            operation_id: receipt.operation_id.clone(),
        });
        // A worker-level progressed record is rejected outright; malformed shape is
        // retained as a not-started receipt when caught by the core validation.
        match result {
            Ok(r) => assert_eq!(r.status, RelocationStatus::NotStarted),
            Err(e) => assert_eq!(e.code, FileErrorCode::InvalidOptions),
        }
        assert!(root.path().join("a").exists() && !root.path().join("new").exists());
        cleanup(&receipt);
    }
}

#[path = "batch_execution_conflict_tests.rs"]
mod conflicts;

#[path = "batch_link_tests.rs"]
mod links;
