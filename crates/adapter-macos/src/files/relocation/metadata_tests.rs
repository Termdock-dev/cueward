use super::tests::fixture;
use super::*;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::process::Command;

use super::execution_tests::{perform, refresh};
#[test]
fn native_move_keeps_mode_mtime_birthtime_and_extended_attribute() {
    let (root, mut request) = fixture();
    let source = root.path().join(&request.path);
    fs::set_permissions(&source, fs::Permissions::from_mode(0o640)).unwrap();
    assert!(
        Command::new("/usr/bin/xattr")
            .args(["-w", "com.cueward.test-relocation", "OWNED metadata"])
            .arg(&source)
            .status()
            .unwrap()
            .success()
    );
    refresh(&mut request);
    let before = source.metadata().unwrap();
    let receipt = perform(&request);
    assert_eq!(
        receipt.status,
        RelocationStatus::Completed,
        "{:?}",
        receipt.error
    );
    let destination = root.path().join(&request.destination);
    let after = destination.metadata().unwrap();
    assert_eq!(
        (
            before.ino(),
            before.mode(),
            before.mtime(),
            before.mtime_nsec(),
            before.created().unwrap()
        ),
        (
            after.ino(),
            after.mode(),
            after.mtime(),
            after.mtime_nsec(),
            after.created().unwrap()
        )
    );
    let xattr = Command::new("/usr/bin/xattr")
        .args(["-p", "com.cueward.test-relocation"])
        .arg(&destination)
        .output()
        .unwrap();
    assert!(xattr.status.success());
    assert_eq!(xattr.stdout, b"OWNED metadata\n");
}
struct Immutable(std::path::PathBuf);
impl Drop for Immutable {
    fn drop(&mut self) {
        let _ = Command::new("/usr/bin/chflags")
            .arg("nouchg")
            .arg(&self.0)
            .status();
    }
}
#[test]
fn permission_denial_keeps_source_and_does_not_create_destination() {
    let (root, mut request) = fixture();
    let immutable = Immutable(root.path().join(&request.path));
    assert!(
        Command::new("/usr/bin/chflags")
            .arg("uchg")
            .arg(&immutable.0)
            .status()
            .unwrap()
            .success()
    );
    refresh(&mut request);
    let receipt = perform(&request);
    assert_eq!(receipt.status, RelocationStatus::NotStarted);
    assert_eq!(receipt.renamed, Some(false));
    assert_eq!(receipt.error.unwrap().code, FileErrorCode::PermissionDenied);
    assert!(immutable.0.exists());
    assert!(!root.path().join(&request.destination).exists());
}
