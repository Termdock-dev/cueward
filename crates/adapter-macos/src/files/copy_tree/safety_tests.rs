use super::tests::{fixture, request};
use super::*;
use cueward_core::files::copy_tree::*;
use cueward_core::files::*;
use std::cell::Cell;
use std::ffi::OsString;
use std::fs::{self, File, Metadata};
use std::io;
use std::os::unix::fs::symlink;

struct Hook {
    action: Box<dyn Fn(&Path)>,
    wide: bool,
    unknown: Option<String>,
    invalid_names: bool,
    package_state: Option<ResourceValue<bool>>,
}
impl FilePlatform for Hook {
    fn stamp(&self, metadata: &Metadata) -> FileStamp {
        let mut stamp = MacFiles.stamp(metadata);
        if self.unknown.as_ref() == Some(&stamp.identity) {
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
        (self.action)(path);
        let mut resources = MacFiles.resource_metadata(path, metadata)?;
        if self.wide {
            resources.finder_tags = ResourceValue::Available {
                value: vec!["x".repeat(65536)],
            };
        }
        if path.ends_with("Owned.app") {
            if let Some(state) = &self.package_state {
                resources.is_package = state.clone();
            }
        }
        Ok(resources)
    }
}
impl CopyTreePlatform for Hook {
    fn open_tree_link(&self, path: &Path) -> Result<File, FileError> {
        MacFiles.open_tree_link(path)
    }
    fn open_tree_directory(&self, path: &Path) -> Result<File, FileError> {
        MacFiles.open_tree_directory(path)
    }
    fn tree_names(&self, file: &File, maximum: usize) -> Result<Vec<OsString>, FileError> {
        if self.invalid_names {
            use std::os::unix::ffi::OsStringExt;
            return Ok(vec![OsString::from_vec(vec![0xff])]);
        }
        MacFiles.tree_names(file, maximum)
    }
    fn validate_tree_metadata(&self, file: &File, info: &FileInfo) -> Result<(), FileError> {
        MacFiles.validate_tree_metadata(file, info)
    }
}
fn hook(action: impl Fn(&Path) + 'static) -> Hook {
    Hook {
        action: Box::new(action),
        wide: false,
        unknown: None,
        invalid_names: false,
        package_state: None,
    }
}
#[test]
fn copy_tree_changes_to_earlier_source_and_destination_are_detected_after_later_inspection() {
    for target in ["source/.hidden", "copy"] {
        let root = fixture();
        let request = request(root.path());
        let selected = root.path().join(target);
        let once = Cell::new(false);
        let platform = hook(move |path| {
            if path.ends_with("sub") && !once.replace(true) {
                fs::write(&selected, b"OWNED external change").unwrap();
            }
        });
        assert_eq!(
            copy_tree::plan(&platform, &request).unwrap_err().code,
            FileErrorCode::Changed
        );
    }
}
#[test]
fn copy_tree_directory_changes_and_parent_replacement_are_detected() {
    for parent in [false, true] {
        let root = fixture();
        fs::create_dir(root.path().join("target-parent")).unwrap();
        let mut request = request(root.path());
        request.destination = "target-parent/copy".into();
        request.expected_parent_version =
            super::super::observe(root.path(), Path::new("target-parent"), false, None)
                .unwrap()
                .version;
        let selected = root
            .path()
            .join(if parent { "target-parent" } else { "source" });
        let once = Cell::new(false);
        let platform = hook(move |path| {
            if path.ends_with("sub") && !once.replace(true) {
                if parent {
                    fs::rename(&selected, selected.with_file_name("saved-parent")).unwrap();
                    fs::create_dir(&selected).unwrap();
                } else {
                    fs::write(selected.join("new-entry"), b"OWNED").unwrap();
                }
            }
        });
        assert_eq!(
            copy_tree::plan(&platform, &request).unwrap_err().code,
            FileErrorCode::Changed
        );
    }
}
#[test]
fn copy_tree_stable_root_alias_uses_initial_canonical_root_and_retarget_is_rejected() {
    let root = fixture();
    let aliases = tempfile::tempdir().unwrap();
    let alias = aliases.path().join("root-link");
    symlink(root.path(), &alias).unwrap();
    let mut request = request(&alias);
    let plan = plan_worker(&request).unwrap();
    assert_eq!(plan.request.root, alias);
    assert_eq!(
        plan.root.path,
        root.path().canonicalize().unwrap().to_str().unwrap()
    );
    let outside = fixture();
    let link = alias.clone();
    let target = outside.path().to_owned();
    let once = Cell::new(false);
    let platform = hook(move |path| {
        if path.ends_with("sub") && !once.replace(true) {
            fs::remove_file(&link).unwrap();
            symlink(&target, &link).unwrap();
        }
    });
    assert_eq!(
        copy_tree::plan(&platform, &request).unwrap_err().code,
        FileErrorCode::Changed
    );
    request.root = root.path().into();
    assert!(plan_worker(&request).is_ok());
}
#[test]
fn copy_tree_unknown_availability_prunes_directories_and_recovery_restores_a_complete_plan() {
    let root = fixture();
    let request = request(root.path());
    let info = super::super::observe(root.path(), Path::new("source/sub"), false, None).unwrap();
    let mut platform = hook(|_| {});
    platform.unknown = Some(info.identity);
    let plan = copy_tree::plan(&platform, &request).unwrap();
    assert!(plan.has_blockers && !plan.enumeration_complete);
    assert!(
        !plan
            .entries
            .iter()
            .any(|e| e.relative_path != Path::new("sub") && e.relative_path.starts_with("sub"))
    );
    platform.unknown = None;
    let plan = copy_tree::plan(&platform, &request).unwrap();
    assert!(!plan.has_blockers && plan.enumeration_complete);
}
#[test]
fn copy_tree_entry_boundary_output_budget_and_descriptor_enumeration_are_bounded() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("source")).unwrap();
    for index in 0..255 {
        fs::write(root.path().join(format!("source/{index:03}")), b"").unwrap();
    }
    let mut selected = request(root.path());
    let plan = plan_worker(&selected).unwrap();
    assert_eq!(plan.entries.len(), 256);
    assert_eq!(plan.observed_file_bytes, 0);
    fs::write(root.path().join("source/overflow"), b"").unwrap();
    selected = request(root.path());
    assert_eq!(
        plan_worker(&selected).unwrap_err().code,
        FileErrorCode::ScanLimit
    );
    fs::remove_file(root.path().join("source/overflow")).unwrap();
    selected = request(root.path());
    let mut platform = hook(|_| {});
    platform.wide = true;
    assert_eq!(
        copy_tree::plan(&platform, &selected).unwrap_err().code,
        FileErrorCode::ScanLimit
    );
    let directory = MacFiles
        .open_tree_directory(&root.path().join("source").canonicalize().unwrap())
        .unwrap();
    assert_eq!(MacFiles.tree_names(&directory, 255).unwrap().len(), 255);
    assert_eq!(MacFiles.tree_names(&directory, 255).unwrap().len(), 255);
    assert_eq!(
        MacFiles.tree_names(&directory, 254).unwrap_err().code,
        FileErrorCode::ScanLimit
    );
}
#[test]
fn copy_tree_guarded_enumeration_keeps_selected_directory_when_name_is_replaced() {
    let root = fixture();
    let selected = root.path().join("source");
    let file = MacFiles
        .open_tree_directory(&selected.canonicalize().unwrap())
        .unwrap();
    fs::rename(&selected, root.path().join("saved-source")).unwrap();
    fs::create_dir(&selected).unwrap();
    fs::write(selected.join("PRIVATE-replacement"), b"not selected").unwrap();
    let names = MacFiles.tree_names(&file, 256).unwrap();
    assert_eq!(names.len(), 2);
    assert!(!names.contains(&OsString::from("PRIVATE-replacement")));
}
#[test]
fn copy_tree_rejects_non_utf8_names_without_lossy_conversion() {
    // APFS rejects invalid UTF-8 at creation; exercise the platform enumeration seam.
    let root = fixture();
    let mut platform = hook(|_| {});
    platform.invalid_names = true;
    assert_eq!(
        copy_tree::plan(&platform, &request(root.path()))
            .unwrap_err()
            .code,
        FileErrorCode::UnsupportedPathEncoding
    );
}
#[test]
fn copy_tree_link_revision_changes_are_rechecked_without_following_targets() {
    let root = fixture();
    symlink("missing", root.path().join("source/a-link")).unwrap();
    let request = request(root.path());
    let selected = root.path().join("source/a-link");
    let once = Cell::new(false);
    let platform = hook(move |path| {
        if path.ends_with("sub") && !once.replace(true) {
            fs::remove_file(&selected).unwrap();
            symlink("different", &selected).unwrap();
        }
    });
    assert_eq!(
        copy_tree::plan(&platform, &request).unwrap_err().code,
        FileErrorCode::Changed
    );
}

#[test]
fn copy_tree_parent_ancestry_uses_identity_not_case_sensitive_path_prefixes() {
    let root = fixture();
    let alternate = root.path().join("SOURCE/sub");
    if !alternate.is_dir() {
        return;
    } // Case-sensitive volumes still run ordinary ancestry tests.
    let mut request = request(root.path());
    request.destination = "SOURCE/sub/copy".into();
    request.expected_parent_version =
        super::super::observe(root.path(), Path::new("SOURCE/sub"), false, None)
            .unwrap()
            .version;
    let plan = plan_worker(&request).unwrap();
    assert!(
        plan.issues
            .contains(&CopyTreeIssue::DestinationWithinSource)
    );
}

#[test]
fn package_opt_in_never_treats_unknown_resource_states_as_known_packages() {
    let root = fixture();
    fs::create_dir(root.path().join("source/Owned.app")).unwrap();
    fs::write(
        root.path().join("source/Owned.app/PRIVATE-descendant"),
        b"not traversed",
    )
    .unwrap();
    let mut selected = request(root.path());
    selected.include_packages = true;
    for state in [
        ResourceValue::Unavailable,
        ResourceValue::Unsupported,
        ResourceValue::NotApplicable,
        ResourceValue::Error {
            error: MetadataError {
                domain: "owned".into(),
                code: 1,
                message: "unknown".into(),
            },
        },
    ] {
        let mut platform = hook(|_| {});
        platform.package_state = Some(state.clone());
        let plan = copy_tree::plan(&platform, &selected).unwrap();
        assert!(plan.has_blockers && !plan.enumeration_complete);
        assert!(
            !plan
                .entries
                .iter()
                .any(|e| e.source.name == "PRIVATE-descendant")
        );
        let entry = plan
            .entries
            .iter()
            .find(|e| e.source.name == "Owned.app")
            .unwrap();
        assert!(entry.error.is_some());
        assert_eq!(entry.resources.as_ref().unwrap().is_package, state);
    }
}
