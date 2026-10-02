use super::*;
use cueward_core::files::relocation::{
    RelocationPlatform, RelocationReceipt, RelocationRequest, RenameOutcome,
};
use cueward_core::files::{FilePlatform, FileStamp, ResourceMetadata};
use std::cell::Cell;
use std::fs::{File, Metadata};
use std::io;

enum Change {
    Source,
    Target,
    Parent,
    Alias(std::path::PathBuf, std::path::PathBuf),
    Final,
    Rejected,
    Unknown,
    Pause(std::path::PathBuf),
}
struct Racing {
    root: std::path::PathBuf,
    calls: Cell<usize>,
    change: Change,
}
impl FilePlatform for Racing {
    fn stamp(&self, m: &Metadata) -> FileStamp {
        MacFiles.stamp(m)
    }
    fn open_regular(&self, p: &Path) -> io::Result<File> {
        MacFiles.open_regular(p)
    }
    fn resource_metadata(&self, p: &Path, m: &Metadata) -> Result<ResourceMetadata, FileError> {
        MacFiles.resource_metadata(p, m)
    }
}
impl BatchRenamePlatform for Racing {
    fn names_may_collide(&self, a: &str, b: &str) -> bool {
        MacFiles.names_may_collide(a, b)
    }
}
impl RelocationPlatform for Racing {
    fn open_directory(&self, p: &Path) -> Result<File, FileError> {
        RelocationPlatform::open_directory(&MacFiles, p)
    }
    fn same_filesystem(&self, a: &Metadata, b: &Metadata) -> bool {
        RelocationPlatform::same_filesystem(&MacFiles, a, b)
    }
    fn prepare_rename(&self, p: &Path, r: &RelocationReceipt) -> Result<(), FileError> {
        RelocationPlatform::prepare_rename(&MacFiles, p, r)
    }
    fn rename(&self, root: &File, r: &RelocationRequest) -> RenameOutcome {
        let count = self.calls.get() + 1;
        self.calls.set(count);
        if matches!(self.change, Change::Rejected | Change::Unknown) {
            return RenameOutcome {
                result: Err(FileError::new(
                    FileErrorCode::Unavailable,
                    "injected native outcome",
                )),
                renamed: if matches!(self.change, Change::Unknown) {
                    None
                } else {
                    Some(false)
                },
            };
        }
        let outcome = RelocationPlatform::rename(&MacFiles, root, r);
        assert_eq!(outcome.renamed, Some(true));
        if count == 1 {
            match &self.change {
                Change::Source => fs::write(self.root.join("b"), b"OWNED changed").unwrap(),
                Change::Target => {
                    fs::write(self.root.join("other"), b"OWNED external target").unwrap()
                }
                Change::Parent => {
                    fs::write(self.root.join("nested/external"), b"OWNED external").unwrap()
                }
                Change::Alias(alias, foreign) => {
                    fs::remove_file(alias).unwrap();
                    symlink(foreign, alias).unwrap();
                }
                Change::Pause(directory) => {
                    fs::write(directory.join("ready"), b"first native rename submitted").unwrap();
                    lifetime::wait_for(|| directory.join("release").exists());
                }
                _ => (),
            }
        }
        if count == 2 && matches!(self.change, Change::Final) {
            fs::write(self.root.join("new"), b"OWNED externally changed result").unwrap();
        }
        outcome
    }
}
#[test]
fn batch_execute_stops_pending_source_target_or_parent_changes_after_one_verified_child() {
    for change in [Change::Source, Change::Target, Change::Parent] {
        let root = fixture();
        fs::create_dir(root.path().join("nested")).unwrap();
        fs::write(root.path().join("nested/c"), b"OWNED c").unwrap();
        let second = if matches!(change, Change::Parent) {
            "nested/c"
        } else {
            "b"
        };
        let platform = Racing {
            root: root.path().into(),
            calls: Cell::new(0),
            change,
        };
        let r = perform(
            &platform,
            &request(root.path(), &[("a", "new"), (second, "other")]),
        );
        assert_eq!(r.status, RelocationStatus::Incomplete, "{:?}", r.error);
        assert!(r.items[0].completion_verified && r.items[1].operation_id.is_none());
        assert_eq!(platform.calls.get(), 1);
        assert_eq!(fs::read(root.path().join("new")).unwrap(), b"OWNED a\0");
        assert!(root.path().join(second).exists());
        if matches!(platform.change, Change::Target) {
            assert_eq!(
                fs::read(root.path().join("other")).unwrap(),
                b"OWNED external target"
            );
        }
        cleanup(&r);
    }
}
#[test]
fn batch_execute_root_alias_retarget_stops_without_touching_foreign_root() {
    let root = fixture();
    let foreign = fixture();
    let aliases = tempfile::tempdir().unwrap();
    let alias = aliases.path().join("root");
    symlink(root.path(), &alias).unwrap();
    let platform = Racing {
        root: root.path().into(),
        calls: Cell::new(0),
        change: Change::Alias(alias.clone(), foreign.path().into()),
    };
    let r = perform(&platform, &request(&alias, &[("a", "new"), ("b", "other")]));
    assert_eq!(r.status, RelocationStatus::Incomplete);
    assert_eq!(platform.calls.get(), 1);
    assert!(r.items[1].operation_id.is_none());
    assert_eq!(fs::read(foreign.path().join("a")).unwrap(), b"OWNED a\0");
    assert_eq!(fs::read(foreign.path().join("b")).unwrap(), b"OWNED b\0");
    assert!(!foreign.path().join("new").exists() && !foreign.path().join("other").exists());
    cleanup(&r);
}
#[test]
fn batch_execute_final_recheck_rejects_changed_completed_destination() {
    let root = fixture();
    let platform = Racing {
        root: root.path().into(),
        calls: Cell::new(0),
        change: Change::Final,
    };
    let r = perform(
        &platform,
        &request(root.path(), &[("a", "new"), ("b", "other")]),
    );
    assert_eq!(r.status, RelocationStatus::Incomplete);
    assert!(!r.completion_verified && r.items.iter().all(|i| i.completion_verified));
    assert_eq!(platform.calls.get(), 2);
    assert_eq!(
        fs::read(root.path().join("new")).unwrap(),
        b"OWNED externally changed result"
    );
    cleanup(&r);
}
#[test]
fn batch_execute_known_native_rejection_and_unknown_submission_never_continue() {
    for change in [Change::Rejected, Change::Unknown] {
        let root = fixture();
        let platform = Racing {
            root: root.path().into(),
            calls: Cell::new(0),
            change,
        };
        let r = perform(
            &platform,
            &request(root.path(), &[("a", "new"), ("b", "other")]),
        );
        assert_eq!(
            r.status,
            if matches!(platform.change, Change::Unknown) {
                RelocationStatus::Uncertain
            } else {
                RelocationStatus::NotStarted
            }
        );
        assert_eq!(platform.calls.get(), 1);
        assert!(r.items[1].operation_id.is_none());
        assert_eq!(fs::read(root.path().join("a")).unwrap(), b"OWNED a\0");
        assert_eq!(fs::read(root.path().join("b")).unwrap(), b"OWNED b\0");
        cleanup(&r);
    }
}
#[test]
fn batch_execute_checkpoint_failure_after_first_child_keeps_partial_evidence_and_stops() {
    let root = fixture();
    let mut receipt = prepare(&request(root.path(), &[("a", "new"), ("b", "other")]));
    let result = execution::execute(
        &MacFiles,
        &mut receipt,
        &mut |r| {
            if r.items[0].completion_verified {
                Err(FileError::new(
                    FileErrorCode::Internal,
                    "injected aggregate checkpoint failure",
                ))
            } else {
                Ok(())
            }
        },
        &mut |i, req, anchor, batch| invoke(&MacFiles, i, req, anchor, batch),
    );
    receipt.record_error(result.unwrap_err());
    save(&receipt).unwrap();
    assert_eq!(receipt.status, RelocationStatus::Incomplete);
    assert!(receipt.items[0].completion_verified && receipt.items[1].operation_id.is_none());
    assert_eq!(fs::read(root.path().join("new")).unwrap(), b"OWNED a\0");
    assert_eq!(fs::read(root.path().join("b")).unwrap(), b"OWNED b\0");
    cleanup(&receipt);
}

#[test]
fn batch_execute_child_anchor_mismatch_refuses_native_submission_with_valid_source_guards() {
    for field in ["path", "identity", "version"] {
        let root = fixture();
        let mut receipt = prepare(&request(root.path(), &[("a", "new"), ("b", "other")]));
        let result = execution::execute(
            &MacFiles,
            &mut receipt,
            &mut save,
            &mut |i, req, anchor, batch| {
                let mut wrong = anchor.clone();
                match field {
                    "path" => wrong.path = "/foreign".into(),
                    "identity" => wrong.identity = "foreign".into(),
                    _ => wrong.version = "stale".into(),
                }
                invoke(&MacFiles, i, req, &wrong, batch)
            },
        );
        receipt.record_error(result.unwrap_err());
        save(&receipt).unwrap();
        assert_eq!(receipt.status, RelocationStatus::NotStarted);
        assert_eq!(
            receipt.items[0].error.as_ref().unwrap().code,
            FileErrorCode::Changed
        );
        assert!(receipt.items[1].operation_id.is_none());
        assert_eq!(fs::read(root.path().join("a")).unwrap(), b"OWNED a\0");
        assert_eq!(fs::read(root.path().join("b")).unwrap(), b"OWNED b\0");
        assert!(!root.path().join("new").exists());
        cleanup(&receipt);
    }
}

#[path = "batch_execution_lifetime_tests.rs"]
mod lifetime;
