use super::tests::fixture;
use super::*;
use std::cell::Cell;
use std::fs;
use std::io;

struct Inspecting {
    change: Box<dyn Fn()>,
    alias: Option<bool>,
    another_filesystem: bool,
    source_available: bool,
    inspected: Cell<bool>,
}
impl FilePlatform for Inspecting {
    fn stamp(&self, metadata: &Metadata) -> FileStamp {
        let mut stamp = MacFiles.stamp(metadata);
        if !self.source_available && metadata.is_file() {
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
        let mut value = MacFiles.resource_metadata(path, metadata)?;
        value.is_alias_file = match self.alias {
            Some(value) => ResourceValue::Available { value },
            None => ResourceValue::Unavailable,
        };
        if !self.inspected.replace(true) {
            (self.change)();
        }
        Ok(value)
    }
}
impl RelocationPlatform for Inspecting {
    fn open_directory(&self, path: &Path) -> Result<File, FileError> {
        MutationPlatform::open_directory(&MacFiles, path)
    }
    fn same_filesystem(&self, source: &Metadata, destination: &Metadata) -> bool {
        !self.another_filesystem
            && RelocationPlatform::same_filesystem(&MacFiles, source, destination)
    }
}
fn inspecting(change: impl Fn() + 'static) -> Inspecting {
    Inspecting {
        change: Box::new(change),
        alias: Some(false),
        another_filesystem: false,
        source_available: true,
        inspected: Cell::new(false),
    }
}
#[test]
fn source_replacement_during_planning_is_rejected_without_moving_either_inode() {
    let (root, request) = fixture();
    let path = root.path().join(&request.path);
    let saved = root.path().join("from/saved-selected");
    let platform = inspecting(move || {
        fs::rename(&path, &saved).unwrap();
        fs::write(&path, b"OWNED external replacement").unwrap();
    });
    assert_eq!(
        relocation::plan(&platform, &request).unwrap_err().code,
        FileErrorCode::Changed
    );
    assert_eq!(
        fs::read(root.path().join("from/saved-selected")).unwrap(),
        b"OWNED\0relocation fixture"
    );
    assert_eq!(
        fs::read(root.path().join(&request.path)).unwrap(),
        b"OWNED external replacement"
    );
    assert!(!root.path().join(&request.destination).exists());
}
#[test]
fn destination_content_change_and_new_conflicts_during_planning_are_rejected() {
    for existing in [false, true] {
        let (root, mut request) = fixture();
        let destination = root.path().join(&request.destination);
        if existing {
            fs::write(&destination, b"OWNED initial destination").unwrap();
        }
        request.expected_parent_version =
            super::super::observe(root.path(), Path::new("to"), false, None)
                .unwrap()
                .version;
        let platform = inspecting(move || {
            fs::write(&destination, b"OWNED external content").unwrap();
        });
        assert_eq!(
            relocation::plan(&platform, &request).unwrap_err().code,
            FileErrorCode::Changed
        );
        assert_eq!(
            fs::read(root.path().join(&request.destination)).unwrap(),
            b"OWNED external content"
        );
        assert!(root.path().join(&request.path).exists());
    }
}
#[test]
fn alias_state_must_be_known_but_alias_relocation_keeps_the_object_opaque() {
    let (_root, request) = fixture();
    let mut platform = inspecting(|| {});
    platform.alias = None;
    assert_eq!(
        relocation::plan(&platform, &request).unwrap_err().code,
        FileErrorCode::Unavailable
    );
    platform.alias = Some(true);
    let plan = relocation::plan(&platform, &request).unwrap();
    assert_eq!(
        plan.source_resources.is_alias_file,
        ResourceValue::Available { value: true }
    );
}

#[test]
fn different_filesystem_is_reported_without_copy_or_source_removal() {
    let (root, request) = fixture();
    let mut platform = inspecting(|| {});
    platform.another_filesystem = true;
    let plan = relocation::plan(&platform, &request).unwrap();
    assert!(!plan.same_filesystem);
    assert_eq!(
        fs::read(root.path().join(&request.path)).unwrap(),
        b"OWNED\0relocation fixture"
    );
    assert!(!root.path().join(&request.destination).exists());
}

#[test]
fn unknown_source_availability_is_rejected_without_materialization() {
    let (root, request) = fixture();
    let mut platform = inspecting(|| {});
    platform.source_available = false;
    assert_eq!(
        relocation::plan(&platform, &request).unwrap_err().code,
        FileErrorCode::Unavailable
    );
    assert!(!platform.inspected.get());
    assert!(root.path().join(&request.path).exists());
    assert!(!root.path().join(&request.destination).exists());
}
