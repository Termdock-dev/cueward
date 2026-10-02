//! Root aliases are supported, but never grant a new scope when retargeted mid-batch.
use super::*;
use cueward_core::files::relocation::RelocationPlatform;
use cueward_core::files::{FileStamp, ResourceMetadata};
use std::cell::{Cell, RefCell};
use std::fs::{File, Metadata};
use std::io;
use std::path::PathBuf;

struct RetargetingRoot {
    anchor_identity: String,
    alias: PathBuf,
    selected: PathBuf,
    foreign: PathBuf,
    anchor_observations: Cell<usize>,
    inspected: RefCell<Vec<PathBuf>>,
}
impl FilePlatform for RetargetingRoot {
    fn stamp(&self, metadata: &Metadata) -> FileStamp {
        let stamp = MacFiles.stamp(metadata);
        if stamp.identity == self.anchor_identity {
            let count = self.anchor_observations.get() + 1;
            self.anchor_observations.set(count);
            // Retarget after the batch's initial root capture, and restore when
            // that original directory is next inspected. Child-directory tokens
            // intentionally belong to the foreign target: versions are not scope.
            let target = match count {
                2 => Some(&self.foreign),
                3 => Some(&self.selected),
                _ => None,
            };
            if let Some(target) = target {
                fs::remove_file(&self.alias).unwrap();
                symlink(target, &self.alias).unwrap();
            }
        }
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
        self.inspected.borrow_mut().push(path.into());
        MacFiles.resource_metadata(path, metadata)
    }
}
impl RelocationPlatform for RetargetingRoot {
    fn open_directory(&self, path: &Path) -> Result<File, FileError> {
        RelocationPlatform::open_directory(&MacFiles, path)
    }
    fn same_filesystem(&self, source: &Metadata, parent: &Metadata) -> bool {
        RelocationPlatform::same_filesystem(&MacFiles, source, parent)
    }
    fn rename(
        &self,
        _: &File,
        _: &cueward_core::files::relocation::RelocationRequest,
    ) -> cueward_core::files::relocation::RenameOutcome {
        panic!("batch root regression must remain read-only");
    }
}
impl BatchRenamePlatform for RetargetingRoot {
    fn names_may_collide(&self, a: &str, b: &str) -> bool {
        MacFiles.names_may_collide(a, b)
    }
}

#[test]
fn batch_rename_restored_root_alias_never_accepts_foreign_root_observations() {
    let selected = fixture();
    let foreign = fixture();
    let aliases = tempfile::tempdir().unwrap();
    let alias = aliases.path().join("root-alias");
    symlink(selected.path(), &alias).unwrap();
    let mut request = request(foreign.path(), &[("a", "new"), ("b", "other")]);
    request.root = alias.clone();
    let selected_path = selected.path().canonicalize().unwrap();
    let platform = RetargetingRoot {
        anchor_identity: MacFiles
            .stamp(&selected.path().metadata().unwrap())
            .identity,
        alias: alias.clone(),
        selected: selected.path().into(),
        foreign: foreign.path().into(),
        anchor_observations: Cell::new(0),
        inspected: RefCell::new(Vec::new()),
    };
    let result = batch::plan(&platform, &request);
    assert!(
        platform.anchor_observations.get() >= 3,
        "retarget/restore interleaving must execute"
    );
    assert_eq!(alias.canonicalize().unwrap(), selected_path);
    if let Ok(plan) = result {
        assert_eq!(plan.root.path, selected_path.to_str().unwrap());
        for item in &plan.items {
            if let Some(proposal) = &item.proposal {
                assert_eq!(
                    proposal.root.path, plan.root.path,
                    "a successful batch must not mix canonical roots"
                );
                assert_eq!(proposal.root.identity, plan.root.identity);
                assert_eq!(proposal.root.version, plan.root.version);
            }
        }
    }
    assert!(
        platform
            .inspected
            .borrow()
            .iter()
            .all(|path| path.starts_with(&selected_path)),
        "foreign root resources must not be inspected through the retargeted alias"
    );
    for root in [selected.path(), foreign.path()] {
        assert_eq!(fs::read(root.join("a")).unwrap(), b"OWNED a\0");
        assert_eq!(fs::read(root.join("b")).unwrap(), b"OWNED b\0");
        assert!(!root.join("new").exists() && !root.join("other").exists());
    }
}

#[test]
fn batch_rename_stable_root_alias_remains_supported_and_preserves_original_request() {
    let selected = fixture();
    let aliases = tempfile::tempdir().unwrap();
    let alias = aliases.path().join("root-alias");
    symlink(selected.path(), &alias).unwrap();
    let request = request(&alias, &[("a", "new"), ("b", "other")]);
    let plan = plan_worker(&request).unwrap();
    assert!(!plan.has_conflicts);
    assert_eq!(plan.request.root, alias);
    for item in &plan.items {
        let proposal = item.proposal.as_ref().unwrap();
        assert_eq!(proposal.root.path, plan.root.path);
        assert_eq!(proposal.root.identity, plan.root.identity);
    }
    assert!(!selected.path().join("new").exists());
}
