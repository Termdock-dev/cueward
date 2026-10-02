use super::*;
use std::ffi::OsStr;
use std::fs::File;

fn direct(
    request: &MutationRequest,
    checkpoint: &mut impl FnMut(&MutationReceipt) -> Result<(), FileError>,
) -> MutationReceipt {
    let mut receipt = journal::create(request).unwrap();
    if let Err(error) = cueward_core::files::mutation::execute(&MacFiles, &mut receipt, checkpoint)
    {
        record_error(&mut receipt, error);
    }
    journal::save(&receipt).unwrap();
    fs::remove_dir_all(journal::directory(&receipt.operation_id).unwrap()).unwrap();
    receipt
}
#[test]
fn destination_appearing_after_preflight_is_not_overwritten() {
    let (root, request) = fixture();
    let receipt = direct(&request, &mut |r| {
        if r.stage == MutationStage::Creating && r.destination_created.is_none() {
            fs::write(root.path().join("copy.txt"), "external bytes").unwrap();
        }
        Ok(())
    });
    assert_eq!(receipt.status, MutationStatus::NotStarted);
    assert_eq!(receipt.destination_created, Some(false));
    assert_eq!(receipt.error.unwrap().code, FileErrorCode::Conflict);
    assert_eq!(
        fs::read(root.path().join("copy.txt")).unwrap(),
        b"external bytes"
    );
}
#[test]
fn checkpoint_failure_after_publication_leaves_verified_bytes_and_preserves_source() {
    let (root, request) = fixture();
    let original = fs::read(root.path().join("source.txt")).unwrap();
    let receipt = direct(&request, &mut |r| {
        if r.destination_created == Some(true) {
            return Err(FileError::new(
                FileErrorCode::Io,
                "owned test checkpoint failure",
            ));
        }
        Ok(())
    });
    assert_eq!(receipt.status, MutationStatus::Incomplete);
    assert!(!receipt.completion_verified);
    assert_eq!(fs::read(root.path().join("source.txt")).unwrap(), original);
    assert_eq!(fs::read(root.path().join("copy.txt")).unwrap(), original);
}

struct ChangingSource {
    path: PathBuf,
}
impl FilePlatform for ChangingSource {
    fn stamp(&self, m: &fs::Metadata) -> FileStamp {
        MacFiles.stamp(m)
    }
    fn open_regular(&self, p: &Path) -> std::io::Result<File> {
        MacFiles.open_regular(p)
    }
}
impl MutationPlatform for ChangingSource {
    fn prepare_staging(
        &self,
        receipt: &MutationReceipt,
        root: &File,
    ) -> Result<Staging, FileError> {
        MacFiles.prepare_staging(receipt, root)
    }
    fn publish(&self, root: &File, staging: &Staging, destination: &Path) -> Publication {
        MacFiles.publish(root, staging, destination)
    }

    fn open_directory(&self, p: &Path) -> Result<File, FileError> {
        MacFiles.open_directory(p)
    }
    fn create_file(&self, d: &File, n: &OsStr) -> Creation {
        MacFiles.create_file(d, n)
    }
    fn create_directory(&self, d: &File, n: &OsStr) -> Creation {
        MacFiles.create_directory(d, n)
    }
    fn validate_copy_source(&self, f: &File, i: &FileInfo) -> Result<(), FileError> {
        MacFiles.validate_copy_source(f, i)
    }
    fn copy_attributes(&self, s: &File, d: &File) -> Result<(String, usize), FileError> {
        let value = MacFiles.copy_attributes(s, d)?;
        fs::write(&self.path, "external modified content")?;
        Ok(value)
    }
}
#[test]
fn external_source_change_after_copy_does_not_claim_completed_or_delete_anything() {
    let (root, request) = fixture();
    let native = ChangingSource {
        path: root.path().join("source.txt"),
    };
    let mut receipt = journal::create(&request).unwrap();
    let error =
        cueward_core::files::mutation::execute(&native, &mut receipt, &mut |_| Ok(())).unwrap_err();
    record_error(&mut receipt, error);
    assert_eq!(receipt.status, MutationStatus::NotStarted);
    assert_eq!(receipt.error.as_ref().unwrap().code, FileErrorCode::Changed);
    assert_eq!(
        fs::read(&native.path).unwrap(),
        b"external modified content"
    );
    assert!(!root.path().join("copy.txt").exists());
    assert!(Path::new(receipt.staging_path.as_ref().unwrap()).is_file());
    journal::save(&receipt).unwrap();
    fs::remove_dir_all(journal::directory(&receipt.operation_id).unwrap()).unwrap();
}

#[test]
fn stopped_worker_produces_persistent_uncertain_receipt_and_never_retries() {
    let (root, mut request) = fixture();
    let executable = root.path().join("worker");
    let destination = root.path().join("copy.txt");
    let quoted = destination.to_str().unwrap().replace('\'', "'\\''");
    fs::write(
        &executable,
        format!("#!/bin/sh\nIFS= read -r input\nprintf partial > '{quoted}'\nexec /bin/sleep 30\n"),
    )
    .unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    fresh(&mut request);
    let receipt = run(&executable, &request, 1000).unwrap();
    assert_eq!(receipt.status, MutationStatus::Uncertain);
    assert_eq!(receipt.error.as_ref().unwrap().code, FileErrorCode::Timeout);
    assert!(!receipt.completion_verified);
    assert_eq!(fs::read(&destination).unwrap(), b"partial");
    assert!(
        read_receipt(&receipt.operation_id)
            .unwrap()
            .error
            .unwrap()
            .message
            .contains("do not automatically retry")
    );
    assert!(
        execute_worker(&MutationWorkerRequest {
            operation_id: receipt.operation_id.clone()
        })
        .is_err()
    );
    assert_eq!(fs::read(destination).unwrap(), b"partial");
    fs::remove_dir_all(journal::directory(&receipt.operation_id).unwrap()).unwrap();
}
#[test]
fn real_completed_operation_cannot_be_replayed_and_receipts_are_private() {
    let (_root, request) = fixture();
    let prepared = journal::create(&request).unwrap();
    let input = MutationWorkerRequest {
        operation_id: prepared.operation_id.clone(),
    };
    assert_eq!(
        execute_worker(&input).unwrap().status,
        MutationStatus::Completed
    );
    assert_eq!(
        execute_worker(&input).unwrap_err().code,
        FileErrorCode::Conflict
    );
    assert_eq!(
        fs::metadata(journal::directory(&input.operation_id).unwrap())
            .unwrap()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(&prepared.receipt_path).unwrap().mode() & 0o777,
        0o600
    );
    assert!(read_receipt("../outside").is_err());
    fs::remove_dir_all(journal::directory(&input.operation_id).unwrap()).unwrap();
}
