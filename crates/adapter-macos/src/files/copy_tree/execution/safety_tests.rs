use super::tests::{direct, fixture, owned, perform, request};
use super::*;
use cueward_core::files::copy_tree::CopyTreePlatform;
use cueward_core::files::mutation::*;
use std::ffi::OsStr;
use std::fs::{self, File, Metadata};
use std::os::unix::fs::symlink;
use std::path::PathBuf;

struct Racing {
    moved_parent: Option<PathBuf>,
    replacement: bool,
    uncertain: bool,
    storage_inside: bool,
    pause: Option<PathBuf>,
}
impl FilePlatform for Racing {
    fn stamp(&self, metadata: &Metadata) -> FileStamp {
        MacFiles.stamp(metadata)
    }
    fn open_regular(&self, path: &Path) -> std::io::Result<File> {
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
impl CopyTreePlatform for Racing {
    fn open_tree_directory(&self, path: &Path) -> Result<File, FileError> {
        MacFiles.open_tree_directory(path)
    }
    fn tree_names(&self, file: &File, max: usize) -> Result<Vec<std::ffi::OsString>, FileError> {
        MacFiles.tree_names(file, max)
    }
    fn validate_tree_metadata(&self, file: &File, info: &FileInfo) -> Result<(), FileError> {
        MacFiles.validate_tree_metadata(file, info)
    }
}
impl TreeExecutionPlatform for Racing {
    fn tree_attribute_digest(&self, file: &File) -> Result<(String, usize), FileError> {
        MacFiles.tree_attribute_digest(file)
    }
}
impl MutationPlatform for Racing {
    fn prepare_staging(
        &self,
        receipt: &MutationReceipt,
        root: &File,
    ) -> Result<Staging, FileError> {
        if self.storage_inside {
            let directory = receipt.request.root.join("source").canonicalize()?;
            return Ok(Staging {
                directory: MacFiles.open_directory(&directory)?,
                path: directory.join("owned-stage"),
            });
        }
        MacFiles.prepare_staging(receipt, root)
    }
    fn publish(&self, root: &File, staged: &Staging, destination: &Path) -> Publication {
        if let Some(parent) = &self.moved_parent {
            fs::rename(
                parent.join("to"),
                parent.parent().unwrap().join("moved-outside"),
            )
            .unwrap();
            if self.replacement {
                fs::create_dir(parent.join("to")).unwrap();
            }
        }
        if self.uncertain {
            return Publication {
                result: Err(FileError::new(
                    FileErrorCode::Io,
                    "owned uncertain submission",
                )),
                destination_created: None,
            };
        }
        MacFiles.publish(root, staged, destination)
    }
    fn open_directory(&self, path: &Path) -> Result<File, FileError> {
        MacFiles.open_directory(path)
    }
    fn create_file(&self, parent: &File, name: &OsStr) -> Creation {
        MacFiles.create_file(parent, name)
    }
    fn create_directory(&self, parent: &File, name: &OsStr) -> Creation {
        MacFiles.create_directory(parent, name)
    }
    fn validate_copy_source(&self, file: &File, info: &FileInfo) -> Result<(), FileError> {
        MacFiles.validate_copy_source(file, info)
    }
    fn copy_attributes(&self, a: &File, b: &File) -> Result<(String, usize), FileError> {
        let digest = MacFiles.copy_attributes(a, b)?;
        if a.metadata()?.is_file() {
            if let Some(control) = &self.pause {
                fs::write(control.join("ready"), b"")?;
                lifetime::wait_for(|| control.join("release").exists());
            }
        }
        Ok(digest)
    }
}
fn racing() -> Racing {
    Racing {
        moved_parent: None,
        replacement: false,
        uncertain: false,
        storage_inside: false,
        pause: None,
    }
}
#[test]
fn tree_execute_publication_never_uses_a_cached_parent_moved_outside_root() {
    for replacement in [false, true] {
        let fixture = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir_in(fixture.path()).unwrap();
        fs::create_dir(root.path().join("source")).unwrap();
        fs::write(root.path().join("source/data"), b"OWNED bytes").unwrap();
        fs::create_dir(root.path().join("to")).unwrap();
        let mut selected = request(root.path());
        selected.destination = "to/destination".into();
        selected.expected_parent_version =
            crate::files::observe(root.path(), Path::new("to"), false, None)
                .unwrap()
                .version;
        let mut value = owned(root, selected);
        let mut platform = racing();
        platform.moved_parent = Some(value.root.path().to_owned());
        platform.replacement = replacement;
        direct(&platform, &mut value, |_| Ok(()));
        assert!(!fixture.path().join("moved-outside/destination").exists());
        assert!(!value.receipt.completion_verified);
        if replacement {
            assert_eq!(value.receipt.status, MutationStatus::Incomplete);
            assert_eq!(
                fs::read(value.root.path().join("to/destination/data")).unwrap(),
                b"OWNED bytes"
            );
        } else {
            assert_eq!(value.receipt.status, MutationStatus::NotStarted);
            assert!(Path::new(value.receipt.staging_path.as_ref().unwrap()).exists());
        }
        assert_eq!(
            fs::read(value.root.path().join("source/data")).unwrap(),
            b"OWNED bytes"
        );
    }
}
#[test]
fn tree_execute_uncertain_submission_keeps_evidence_and_never_retries() {
    let root = fixture();
    let selected = request(root.path());
    let mut value = owned(root, selected);
    let mut platform = racing();
    platform.uncertain = true;
    direct(&platform, &mut value, |_| Ok(()));
    assert_eq!(value.receipt.status, MutationStatus::Uncertain);
    assert_eq!(value.receipt.destination_created, None);
    assert!(value.receipt.mutation_attempted && value.receipt.staging_verified);
    assert!(Path::new(value.receipt.staging_path.as_ref().unwrap()).exists());
    assert!(!value.root.path().join("destination").exists());
}
#[test]
fn tree_execute_private_storage_inside_source_is_refused_before_staged_payloads() {
    let root = fixture();
    let selected = request(root.path());
    let mut value = owned(root, selected);
    let mut platform = racing();
    platform.storage_inside = true;
    direct(&platform, &mut value, |_| Ok(()));
    assert_eq!(value.receipt.status, MutationStatus::NotStarted);
    assert!(!value.root.path().join("source/owned-stage").exists());
    assert!(value.receipt.staging_path.is_none());
}
#[test]
fn tree_execute_initial_manifest_overflow_refuses_without_oversized_saved_evidence() {
    let root = tempfile::tempdir().unwrap();
    let mut directory = root.path().join("source");
    fs::create_dir(&directory).unwrap();
    for index in 0..3 {
        directory = directory.join(format!("{}{}", "n\n".repeat(90), index));
        fs::create_dir(&directory).unwrap();
    }
    for index in 0..60 {
        fs::write(
            directory.join(format!("{}{}", "f\n".repeat(90), index)),
            b"",
        )
        .unwrap();
    }
    let selected = request(root.path());
    let mut value = owned(root, selected);
    perform(&mut value);
    assert_eq!(
        value.receipt.status,
        MutationStatus::NotStarted,
        "{:?}",
        value.receipt.error
    );
    assert_eq!(
        value.receipt.error.as_ref().unwrap().code,
        FileErrorCode::ScanLimit
    );
    assert_eq!(value.receipt.preflight_nodes_omitted, 64);
    assert!(value.receipt.nodes.is_empty() && value.receipt.staging_path.is_none());
    assert!(fs::metadata(&value.receipt.receipt_path).unwrap().len() < 256 * 1024);
    assert_eq!(
        serde_json::to_value(read_receipt(&value.receipt.operation_id).unwrap()).unwrap(),
        serde_json::to_value(&value.receipt).unwrap()
    );
}
#[test]
fn tree_execute_count_boundary_completes_and_refuses_nonfresh_worker_records() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("source")).unwrap();
    for index in 0..63 {
        fs::write(root.path().join(format!("source/{index:02}")), b"").unwrap();
    }
    let selected = request(root.path());
    let mut value = owned(root, selected);
    perform(&mut value);
    assert_eq!(
        value.receipt.status,
        MutationStatus::Completed,
        "{:?}",
        value.receipt.error
    );
    assert_eq!(value.receipt.nodes.len(), 64);
    assert!(fs::metadata(&value.receipt.receipt_path).unwrap().len() < 256 * 1024);
    let root = fixture();
    let selected = request(root.path());
    let mut value = owned(root, selected);
    value.receipt.preflight_nodes_omitted = 1;
    save(&value.receipt).unwrap();
    assert!(
        execute_worker(&TreeWorkerRequest {
            operation_id: value.receipt.operation_id.clone()
        })
        .is_err()
    );
    assert!(!value.root.path().join("destination").exists());
}
#[test]
fn tree_execute_aggregate_xattr_budget_is_checked_before_any_staged_creation() {
    use std::os::fd::AsRawFd;
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("source")).unwrap();
    let bytes = vec![b'x'; 3 * 1024 * 1024];
    for index in 0..3 {
        let file = File::create(root.path().join(format!("source/{index}"))).unwrap();
        // SAFETY: valid owned fixture descriptor, constant name and bounded readable vector.
        assert_eq!(
            unsafe {
                libc::fsetxattr(
                    file.as_raw_fd(),
                    c"com.cueward.owned-budget".as_ptr(),
                    bytes.as_ptr().cast(),
                    bytes.len(),
                    0,
                    0,
                )
            },
            0
        );
    }
    let selected = request(root.path());
    let mut value = owned(root, selected);
    perform(&mut value);
    assert_eq!(value.receipt.status, MutationStatus::NotStarted);
    assert_eq!(
        value.receipt.error.as_ref().unwrap().code,
        FileErrorCode::ScanLimit
    );
    assert!(value.receipt.staging_path.is_none());
}
#[test]
fn tree_execute_root_alias_stable_then_changed_mapping_preserves_private_copy() {
    for change in [false, true] {
        let root = fixture();
        let aliases = tempfile::tempdir().unwrap();
        let outside = fixture();
        let alias = aliases.path().join("selected");
        symlink(root.path(), &alias).unwrap();
        let selected = request(&alias);
        let mut value = owned(root, selected);
        direct(&MacFiles, &mut value, |r| {
            if change && r.stage == TreeStage::Staged {
                fs::remove_file(&alias).unwrap();
                symlink(outside.path(), &alias).unwrap();
            }
            Ok(())
        });
        assert_eq!(
            value.receipt.status,
            if change {
                MutationStatus::NotStarted
            } else {
                MutationStatus::Completed
            }
        );
        assert!(!outside.path().join("destination").exists());
    }
}

#[path = "lifetime_tests.rs"]
mod lifetime;
