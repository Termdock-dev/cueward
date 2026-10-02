use super::*;
use cueward_core::files::relocation::RelocationPlatform;
use cueward_core::files::{DataState, FileStamp, ResourceMetadata, ResourceValue};
use std::cell::Cell;
use std::fs::{File, Metadata};
use std::io;

struct Inspecting {
    calls: Cell<usize>,
    change: Box<dyn Fn()>,
    alias: Option<bool>,
    available: bool,
    same_device: bool,
}
impl FilePlatform for Inspecting {
    fn stamp(&self, metadata: &Metadata) -> FileStamp {
        let mut stamp = MacFiles.stamp(metadata);
        if !self.available && metadata.is_file() {
            stamp.data_state = DataState::Unknown;
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
        let calls = self.calls.get() + 1;
        self.calls.set(calls);
        if calls == 2 {
            (self.change)();
        }
        let mut resources = MacFiles.resource_metadata(path, metadata)?;
        resources.is_alias_file = self
            .alias
            .map(|value| ResourceValue::Available { value })
            .unwrap_or(ResourceValue::Unavailable);
        Ok(resources)
    }
}
impl RelocationPlatform for Inspecting {
    fn open_directory(&self, path: &Path) -> Result<File, FileError> {
        RelocationPlatform::open_directory(&MacFiles, path)
    }
    fn same_filesystem(&self, _: &Metadata, _: &Metadata) -> bool {
        self.same_device
    }
    fn rename(
        &self,
        _: &File,
        _: &cueward_core::files::relocation::RelocationRequest,
    ) -> cueward_core::files::relocation::RenameOutcome {
        panic!("read-only batch planning must not rename");
    }
}
impl BatchRenamePlatform for Inspecting {
    fn names_may_collide(&self, a: &str, b: &str) -> bool {
        MacFiles.names_may_collide(a, b)
    }
}
fn inspecting(change: impl Fn() + 'static) -> Inspecting {
    Inspecting {
        calls: Cell::new(0),
        change: Box::new(change),
        alias: Some(false),
        available: true,
        same_device: true,
    }
}
#[test]
fn batch_rename_rechecks_earlier_sources_after_later_proposals_are_observed() {
    let root = fixture();
    let request = request(root.path(), &[("a", "new"), ("b", "other")]);
    let path = root.path().join("a");
    let platform = inspecting(move || {
        fs::write(&path, b"OWNED externally changed a").unwrap();
    });
    assert_eq!(
        batch::plan(&platform, &request).unwrap_err().code,
        FileErrorCode::Changed
    );
    assert!(!root.path().join("new").exists());
    assert_eq!(
        fs::read(root.path().join("a")).unwrap(),
        b"OWNED externally changed a"
    );
}
#[test]
fn batch_rename_rechecks_destination_conflicts_that_change_between_items() {
    let root = fixture();
    fs::write(
        root.path().join("nested/existing"),
        b"OWNED old destination",
    )
    .unwrap();
    let request = request(root.path(), &[("nested/c", "existing"), ("b", "other")]);
    let path = root.path().join("nested/existing");
    let platform = inspecting(move || {
        fs::write(&path, b"OWNED new destination").unwrap();
    });
    assert_eq!(
        batch::plan(&platform, &request).unwrap_err().code,
        FileErrorCode::Changed
    );
    assert_eq!(
        fs::read(root.path().join("nested/existing")).unwrap(),
        b"OWNED new destination"
    );
    assert!(root.path().join("nested/c").exists());
}
#[test]
fn batch_rename_alias_and_availability_failures_are_explicit_item_errors() {
    let root = fixture();
    let request = request(root.path(), &[("a", "new")]);
    for (alias, available, code) in [
        (Some(true), true, FileErrorCode::UnsupportedType),
        (None, true, FileErrorCode::Unavailable),
        (Some(false), false, FileErrorCode::Unavailable),
    ] {
        let mut platform = inspecting(|| {});
        platform.alias = alias;
        platform.available = available;
        let plan = batch::plan(&platform, &request).unwrap();
        assert!(plan.has_conflicts);
        assert_eq!(plan.items[0].error.as_ref().unwrap().code, code);
        assert!(plan.items[0].proposal.is_none());
    }
    assert!(root.path().join("a").exists());
    assert!(!root.path().join("new").exists());
}
#[test]
fn batch_rename_different_device_observation_is_not_a_copy_fallback() {
    let root = fixture();
    let request = request(root.path(), &[("a", "new")]);
    let mut platform = inspecting(|| {});
    platform.same_device = false;
    let plan = batch::plan(&platform, &request).unwrap();
    assert!(plan.has_conflicts && has(&plan, BatchRenameIssueCode::DifferentFilesystem, &[0]));
    assert!(root.path().join("a").exists());
    assert!(!root.path().join("new").exists());
}
