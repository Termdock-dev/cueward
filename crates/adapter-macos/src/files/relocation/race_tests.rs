use super::tests::fixture;
use super::*;
use std::cell::Cell;
use std::fs;
use std::os::unix::fs::symlink;

struct Boundary {
    mode: u8,
    called: Cell<bool>,
}
impl FilePlatform for Boundary {
    fn stamp(&self, m: &Metadata) -> FileStamp {
        MacFiles.stamp(m)
    }
    fn open_regular(&self, p: &Path) -> std::io::Result<File> {
        MacFiles.open_regular(p)
    }
    fn resource_metadata(&self, p: &Path, m: &Metadata) -> Result<ResourceMetadata, FileError> {
        MacFiles.resource_metadata(p, m)
    }
}
impl RelocationPlatform for Boundary {
    fn open_directory(&self, p: &Path) -> Result<File, FileError> {
        RelocationPlatform::open_directory(&MacFiles, p)
    }
    fn same_filesystem(&self, s: &Metadata, d: &Metadata) -> bool {
        self.mode != 4 && MacFiles.same_filesystem(s, d)
    }
    fn prepare_rename(&self, p: &Path, r: &RelocationReceipt) -> Result<(), FileError> {
        native::prepare(p, r)
    }
    fn rename(&self, root: &File, r: &RelocationRequest) -> relocation::RenameOutcome {
        self.called.set(true);
        let source = r.root.join(&r.path);
        replace_at_boundary(self.mode, r, &source);
        let outcome = native::rename(root, r);
        if self.mode == 8 {
            fs::rename(r.root.join("to"), r.root.join("saved-parent")).unwrap();
            fs::create_dir(r.root.join("to")).unwrap();
            fs::write(r.root.join(&r.destination), b"OWNED post-call replacement").unwrap();
        }
        outcome
    }
}
fn execute_boundary(request: &RelocationRequest, mode: u8) -> (RelocationReceipt, bool) {
    let mut receipt = STORE
        .create(|id, path| RelocationReceipt::new(id, path, request.clone()))
        .unwrap();
    let platform = Boundary {
        mode,
        called: Cell::new(false),
    };
    if let Err(error) = relocation::execute(&platform, &mut receipt, &mut transport::save) {
        receipt.record_error(error);
    }
    (receipt, platform.called.get())
}
#[test]
fn late_destination_conflict_is_rejected_by_kernel_without_overwrite() {
    for mode in [0, 5, 6] {
        let (root, request) = fixture();
        let (receipt, called) = execute_boundary(&request, mode);
        assert!(called);
        assert_eq!(receipt.status, RelocationStatus::NotStarted);
        assert_eq!(receipt.renamed, Some(false));
        assert_eq!(receipt.error.unwrap().code, FileErrorCode::Conflict);
        assert!(receipt.destination_after.is_some());
        if mode == 0 {
            assert_eq!(
                fs::read(root.path().join(&request.destination)).unwrap(),
                b"OWNED late conflict"
            );
        }
        assert!(root.path().join(&request.path).exists());
    }
}
#[test]
fn destination_parent_link_replacement_is_not_followed_at_submission() {
    let (root, request) = fixture();
    let (receipt, called) = execute_boundary(&request, 7);
    assert!(called);
    assert_eq!(receipt.renamed, Some(false));
    assert_eq!(
        receipt.error.unwrap().code,
        FileErrorCode::SymlinkDisallowed
    );
    assert!(!root.path().join("saved-parent/renamed").exists());
    assert!(root.path().join(&request.path).exists());
}
#[test]
fn source_replaced_after_last_check_is_reported_incomplete_and_never_rolled_back() {
    for mode in [1, 2] {
        let (root, request) = fixture();
        let (receipt, called) = execute_boundary(&request, mode);
        assert!(called);
        assert_eq!(receipt.status, RelocationStatus::Incomplete);
        assert_eq!(receipt.renamed, Some(true));
        assert!(!receipt.completion_verified);
        assert_ne!(
            receipt.destination_after.unwrap().identity,
            receipt.before.unwrap().source.identity
        );
        assert_eq!(
            fs::read(root.path().join("saved-original")).unwrap(),
            b"OWNED\0relocation fixture"
        );
        if mode == 1 {
            assert_eq!(
                fs::read(root.path().join(&request.destination)).unwrap(),
                b"OWNED replacement"
            );
        } else {
            assert!(
                fs::symlink_metadata(root.path().join(&request.destination))
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
        }
    }
}
#[test]
fn moved_destination_parent_does_not_redirect_submission_to_cached_parent_descriptor() {
    let (root, request) = fixture();
    let outside = root.path().parent().unwrap().join(format!(
        "owned-outside-{}",
        root.path().file_name().unwrap().to_str().unwrap()
    ));
    let (receipt, called) = execute_boundary(&request, 3);
    // Restore only the newly owned empty fixture directory for test teardown.
    fs::rename(&outside, root.path().join("to")).unwrap();
    assert!(called);
    assert_eq!(receipt.renamed, Some(false));
    assert_eq!(receipt.error.unwrap().code, FileErrorCode::NotFound);
    assert!(!root.path().join(&request.destination).exists());
    assert!(root.path().join(&request.path).exists());
}
#[test]
fn cross_volume_observation_fails_without_native_submission_or_copy_delete() {
    let (root, request) = fixture();
    let (receipt, called) = execute_boundary(&request, 4);
    assert!(!called);
    assert_eq!(receipt.status, RelocationStatus::NotStarted);
    assert_eq!(receipt.error.unwrap().code, FileErrorCode::Unavailable);
    assert!(root.path().join(&request.path).exists());
    assert!(!root.path().join(&request.destination).exists());
}
#[test]
fn failure_after_submission_preserves_the_moved_object_and_classifies_incomplete() {
    let (root, request) = fixture();
    let mut receipt = STORE
        .create(|id, path| RelocationReceipt::new(id, path, request.clone()))
        .unwrap();
    let error = relocation::execute(&MacFiles, &mut receipt, &mut |r| {
        if r.stage == RelocationStage::Verifying {
            Err(FileError::new(
                FileErrorCode::Io,
                "owned post-submission journal failure",
            ))
        } else {
            transport::save(r)
        }
    })
    .unwrap_err();
    receipt.record_error(error);
    assert_eq!(receipt.status, RelocationStatus::Incomplete);
    assert_eq!(receipt.renamed, Some(true));
    assert!(root.path().join(&request.destination).exists());
    assert!(!root.path().join(&request.path).exists());
}

#[test]
fn post_call_parent_replacement_preserves_both_objects_and_reports_actual_destination() {
    let (root, request) = fixture();
    let (receipt, called) = execute_boundary(&request, 8);
    assert!(called);
    assert_eq!(receipt.status, RelocationStatus::Incomplete);
    assert_eq!(receipt.renamed, Some(true));
    assert!(!receipt.completion_verified);
    assert_ne!(
        receipt.destination_parent_after.unwrap().identity,
        receipt.before.unwrap().destination_parent.identity
    );
    assert_eq!(
        fs::read(root.path().join("saved-parent/renamed")).unwrap(),
        b"OWNED\0relocation fixture"
    );
    assert_eq!(
        fs::read(root.path().join(&request.destination)).unwrap(),
        b"OWNED post-call replacement"
    );
}
#[test]
fn stopped_worker_returns_uncertain_saved_evidence_without_relocating_or_retrying() {
    use std::os::unix::fs::PermissionsExt;
    let (root, request) = fixture();
    let executable = root.path().join("owned-blocking-worker");
    fs::write(&executable, "#!/bin/sh\nexec /bin/sleep 30\n").unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    let receipt = run(&executable, &request, 100).unwrap();
    assert_eq!(receipt.status, RelocationStatus::Uncertain);
    assert_eq!(receipt.error.as_ref().unwrap().code, FileErrorCode::Timeout);
    assert!(!receipt.completion_verified);
    assert_eq!(
        read_receipt(&receipt.operation_id).unwrap().status,
        RelocationStatus::Uncertain
    );
    assert!(root.path().join(&request.path).exists());
    assert!(!root.path().join(&request.destination).exists());
}

fn replace_at_boundary(mode: u8, r: &RelocationRequest, source: &Path) {
    match mode {
        5 => symlink("missing-owned-target", r.root.join(&r.destination)).unwrap(),
        6 => fs::create_dir(r.root.join(&r.destination)).unwrap(),
        7 => {
            fs::rename(r.root.join("to"), r.root.join("saved-parent")).unwrap();
            symlink("saved-parent", r.root.join("to")).unwrap();
        }
        0 => fs::write(r.root.join(&r.destination), b"OWNED late conflict").unwrap(),
        1 | 2 => {
            fs::rename(source, r.root.join("saved-original")).unwrap();
            if mode == 1 {
                fs::write(source, b"OWNED replacement").unwrap();
            } else {
                symlink("../saved-original", source).unwrap();
            }
        }
        3 => {
            fs::rename(
                r.root.join("to"),
                r.root.parent().unwrap().join(format!(
                    "owned-outside-{}",
                    r.root.file_name().unwrap().to_str().unwrap()
                )),
            )
            .unwrap();
        }
        _ => {}
    }
}
