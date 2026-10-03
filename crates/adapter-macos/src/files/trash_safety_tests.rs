use super::tests::request;
use super::*;
use crate::files::FilePlatform;
use cueward_core::files::trash::{TrashTargetKind as Kind, TrashWarning as Warning};
use cueward_core::files::*;
use std::cell::Cell;
use std::fs::{self, File, Metadata};
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use std::path::PathBuf;

struct Observing {
    inode: u64,
    state: Option<DataState>,
    resources: Option<ResourceMetadata>,
    change: Option<Box<dyn Fn()>>,
    calls: Cell<usize>,
}
impl FilePlatform for Observing {
    fn stamp(&self, metadata: &Metadata) -> FileStamp {
        let mut result = MacFiles.stamp(metadata);
        if metadata.ino() == self.inode {
            if let Some(state) = &self.state {
                result.data_state = state.clone();
            }
        }
        result
    }
    fn open_regular(&self, _: &Path) -> std::io::Result<File> {
        panic!("trash planning must never open payloads")
    }
    fn resource_metadata(
        &self,
        path: &Path,
        metadata: &Metadata,
    ) -> Result<ResourceMetadata, FileError> {
        self.calls.set(self.calls.get() + 1);
        if let Some(change) = &self.change {
            change();
        }
        if let Some(resources) = &self.resources {
            Ok(resources.clone())
        } else {
            MacFiles.resource_metadata(path, metadata)
        }
    }
}
fn platform(path: &Path) -> Observing {
    Observing {
        inode: fs::symlink_metadata(path).unwrap().ino(),
        state: None,
        resources: None,
        change: None,
        calls: Cell::new(0),
    }
}
fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("sub")).unwrap();
    fs::write(root.path().join("sub/owned"), b"OWNED payload").unwrap();
    root
}
#[test]
fn trash_plan_unknown_and_dataless_availability_is_evidence_not_a_download_or_empty_file() {
    for state in [DataState::Unknown, DataState::Dataless] {
        let root = fixture();
        let selected = request(root.path(), "sub/owned");
        let mut selected_platform = platform(&root.path().join("sub/owned"));
        selected_platform.state = Some(state.clone());
        let result = trash::plan(&selected_platform, &selected).unwrap();
        let expected = if state == DataState::Unknown {
            Warning::SourceAvailabilityUnknown
        } else {
            Warning::SourceDataless
        };
        assert!(result.warnings.contains(&expected));
        if state == DataState::Dataless {
            assert_eq!(selected_platform.calls.get(), 0);
            assert_eq!(result.resources.is_alias_file, ResourceValue::Unavailable);
            assert_eq!(result.target_kind, Kind::Unknown);
        }
        assert_eq!(
            fs::read(root.path().join("sub/owned")).unwrap(),
            b"OWNED payload"
        );
    }
}
#[test]
fn trash_plan_resource_alias_and_unknown_error_fields_are_retained_without_resolution() {
    let root = fixture();
    let selected = request(root.path(), "sub/owned");
    for alias in [
        ResourceValue::Available { value: true },
        ResourceValue::Unavailable,
        ResourceValue::Error {
            error: MetadataError {
                domain: "OWNED".into(),
                code: 7,
                message: "owned error".into(),
            },
        },
    ] {
        let mut resources = ResourceMetadata::unsupported();
        resources.is_alias_file = alias.clone();
        let mut selected_platform = platform(&root.path().join("sub/owned"));
        selected_platform.resources = Some(resources);
        let result = trash::plan(&selected_platform, &selected).unwrap();
        assert_eq!(result.resources.is_alias_file, alias);
        if matches!(alias, ResourceValue::Available { value: true }) {
            assert_eq!(result.target_kind, Kind::FinderAlias);
            assert!(result.warnings.contains(&Warning::AliasFileItself));
        } else {
            assert_eq!(result.target_kind, Kind::Unknown);
            assert!(result.warnings.contains(&Warning::AliasStateUnknown));
        }
        assert!(!result.link_targets_inspected && !result.execution_supported);
    }
}
#[test]
fn trash_plan_unknown_directory_package_state_never_claims_an_ordinary_directory() {
    let root = fixture();
    let selected = request(root.path(), "sub");
    let mut selected_platform = platform(&root.path().join("sub"));
    selected_platform.resources = Some(ResourceMetadata::unsupported());
    let result = trash::plan(&selected_platform, &selected).unwrap();
    assert_eq!(result.target_kind, Kind::Unknown);
    assert!(
        result.warnings.contains(&Warning::WholeDirectoryEntry)
            && result.warnings.contains(&Warning::PackageStateUnknown)
    );
}
#[test]
fn trash_plan_source_and_parent_changes_during_resource_inspection_discard_the_proposal() {
    for mode in ["source", "parent", "replace"] {
        let root = fixture();
        let selected = request(root.path(), "sub/owned");
        let mut selected_platform = platform(&root.path().join("sub/owned"));
        let path = root.path().to_owned();
        selected_platform.change = Some(Box::new(move || match mode {
            "source" => fs::write(path.join("sub/owned"), b"OWNED changed").unwrap(),
            "parent" => fs::write(path.join("sub/new"), b"OWNED new").unwrap(),
            _ => {
                fs::rename(path.join("sub/owned"), path.join("sub/parked")).unwrap();
                fs::write(path.join("sub/owned"), b"OWNED replacement").unwrap();
            }
        }));
        assert_eq!(
            trash::plan(&selected_platform, &selected).unwrap_err().code,
            FileErrorCode::Changed
        );
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}
#[test]
fn trash_plan_root_mapping_changed_during_inspection_cannot_return_replacement_observations() {
    let root = fixture();
    let replacement = fixture();
    let aliases = tempfile::tempdir().unwrap();
    let alias = aliases.path().join("root");
    symlink(root.path(), &alias).unwrap();
    let selected = request(&alias, "sub/owned");
    let mut selected_platform = platform(&root.path().join("sub/owned"));
    let mapping = alias.clone();
    let outside = replacement.path().to_owned();
    selected_platform.change = Some(Box::new(move || {
        fs::remove_file(&mapping).unwrap();
        symlink(&outside, &mapping).unwrap();
    }));
    assert_eq!(
        trash::plan(&selected_platform, &selected).unwrap_err().code,
        FileErrorCode::Changed
    );
    assert_eq!(
        fs::read(replacement.path().join("sub/owned")).unwrap(),
        b"OWNED payload"
    );
}
#[test]
fn trash_plan_parent_readonly_and_unknown_availability_do_not_certify_delete_permissions() {
    let root = fixture();
    let selected = request(root.path(), "sub/owned");
    fs::set_permissions(root.path().join("sub"), fs::Permissions::from_mode(0o555)).unwrap();
    let mut selected_platform = platform(&root.path().join("sub"));
    selected_platform.state = Some(DataState::Unknown);
    let result = trash::plan(&selected_platform, &selected).unwrap();
    assert!(
        result.warnings.contains(&Warning::ParentReadonlyMode)
            && result
                .warnings
                .contains(&Warning::ParentAvailabilityUnknown)
    );
    assert!(!result.execution_supported);
    fs::set_permissions(root.path().join("sub"), fs::Permissions::from_mode(0o700)).unwrap();
}
#[test]
fn trash_plan_fixed_json_budget_returns_an_error_not_partial_metadata() {
    let root = fixture();
    let selected = request(root.path(), "sub/owned");
    let mut selected_platform = platform(&root.path().join("sub/owned"));
    let mut resources = ResourceMetadata::unsupported();
    resources.finder_tags = ResourceValue::Available {
        value: vec!["x".repeat(65536)],
    };
    selected_platform.resources = Some(resources);
    assert_eq!(
        trash::plan(&selected_platform, &selected).unwrap_err().code,
        FileErrorCode::ScanLimit
    );
    assert_eq!(
        fs::read(root.path().join("sub/owned")).unwrap(),
        b"OWNED payload"
    );
}
#[test]
fn trash_plan_dedicated_worker_deadline_and_request_schema_are_bounded() {
    let root = fixture();
    let selected = request(root.path(), "sub/owned");
    let workers = tempfile::tempdir().unwrap();
    let worker = workers.path().join("worker");
    fs::write(&worker, "#!/bin/sh\nexec /bin/sleep 30\n").unwrap();
    fs::set_permissions(&worker, fs::Permissions::from_mode(0o700)).unwrap();
    let start = std::time::Instant::now();
    assert_eq!(
        run_plan(&worker, &selected, 100).unwrap_err().code,
        FileErrorCode::Timeout
    );
    assert!(start.elapsed() < std::time::Duration::from_secs(5));
    assert_eq!(
        run_plan(&worker, &selected, 0).unwrap_err().code,
        FileErrorCode::InvalidOptions
    );
    let mut invalid = serde_json::to_value(&selected).unwrap();
    invalid["confirm"] = true.into();
    assert!(serde_json::from_value::<TrashPlanRequest>(invalid).is_err());
    let huge = TrashPlanRequest {
        root: PathBuf::from(format!("/{}", "x".repeat(16384))),
        ..selected
    };
    assert_eq!(
        plan_worker(&huge).unwrap_err().code,
        FileErrorCode::InvalidOptions
    );
}
