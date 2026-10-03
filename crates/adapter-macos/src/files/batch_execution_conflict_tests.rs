use super::*;
use cueward_core::files::relocation::{
    RelocationPlatform, RelocationReceipt, RelocationRequest, RenameOutcome,
};
use cueward_core::files::{FilePlatform, FileStamp, ResourceMetadata};
use std::fs::{File, Metadata};
use std::hash::{Hash, Hasher};
use std::io;

struct ShortRevisions;
impl FilePlatform for ShortRevisions {
    fn stamp(&self, metadata: &Metadata) -> FileStamp {
        let mut stamp = MacFiles.stamp(metadata);
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        stamp.version.hash(&mut hash);
        stamp.version = hash.finish().to_string();
        stamp
    }
    fn open_regular(&self, path: &Path) -> io::Result<File> {
        MacFiles.open_regular(path)
    }
    fn resource_metadata(
        &self,
        path: &Path,
        metadata: &Metadata,
    ) -> Result<ResourceMetadata, FileError> {
        MacFiles.resource_metadata(path, metadata)
    }
}
impl BatchRenamePlatform for ShortRevisions {
    fn names_may_collide(&self, a: &str, b: &str) -> bool {
        MacFiles.names_may_collide(a, b)
    }
}
impl RelocationPlatform for ShortRevisions {
    fn open_directory(&self, path: &Path) -> Result<File, FileError> {
        RelocationPlatform::open_directory(&MacFiles, path)
    }
    fn same_filesystem(&self, a: &Metadata, b: &Metadata) -> bool {
        RelocationPlatform::same_filesystem(&MacFiles, a, b)
    }
    fn prepare_rename(&self, _: &Path, _: &RelocationReceipt) -> Result<(), FileError> {
        panic!("dense conflicts must not prepare a rename")
    }
    fn rename(&self, _: &File, _: &RelocationRequest) -> RenameOutcome {
        panic!("dense conflicts must not submit a rename")
    }
}
#[test]
fn batch_execute_count_ceiling_conflicts_save_not_started_without_any_child() {
    let root = fixture();
    let mut request = request(root.path(), &[("a", "a")]);
    request.entries[0].expected_version = ShortRevisions
        .stamp(&root.path().join("a").metadata().unwrap())
        .version;
    request.entries[0].expected_parent_version = ShortRevisions
        .stamp(&root.path().metadata().unwrap())
        .version;
    request.entries = vec![request.entries[0].clone(); 64];
    assert!(serde_json::to_vec(&request).unwrap().len() <= 16384);
    let prepared = prepare(&request);
    let result = execute_prepared(
        &ShortRevisions,
        &BatchExecutionWorkerRequest {
            operation_id: prepared.operation_id.clone(),
        },
    );
    let stored = read_receipt(&prepared.operation_id).unwrap();
    let bytes = fs::metadata(&prepared.receipt_path).unwrap().len();
    cleanup(&stored);
    let receipt =
        result.expect("every accepted dense-conflict request must save authoritative evidence");
    let value = serde_json::to_value(&receipt).unwrap();
    assert_eq!(receipt.status, RelocationStatus::NotStarted);
    assert_eq!(
        receipt.error.as_ref().unwrap().code,
        FileErrorCode::Conflict
    );
    assert_eq!(receipt.items.len(), 64);
    assert!(
        receipt
            .items
            .iter()
            .all(|i| i.operation_id.is_none() && !i.mutation_attempted)
    );
    assert!(value["omitted_issue_count"].as_u64().unwrap() > 0);
    assert_eq!(
        receipt.issues.len() + value["omitted_issue_count"].as_u64().unwrap() as usize,
        3 * 64 * 63 / 2
    );
    assert_eq!(serde_json::to_value(&stored).unwrap(), value);
    assert!(bytes <= 256 * 1024);
    assert_eq!(fs::read(root.path().join("a")).unwrap(), b"OWNED a\0");
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
}
