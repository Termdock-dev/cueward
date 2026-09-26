#[cfg(test)]
use std::fs::{File, OpenOptions};
#[cfg(test)]
use std::os::fd::AsRawFd;
#[cfg(test)]
use std::os::unix::fs::OpenOptionsExt;
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;

use crate::MacosError;
use crate::screenshot::ensure_cache_dir;

#[cfg(test)]
unsafe extern "C" {
    fn flock(fd: std::ffi::c_int, operation: std::ffi::c_int) -> std::ffi::c_int;
}

pub(super) fn input_lock_path(pid: i32) -> Result<PathBuf, MacosError> {
    Ok(PathBuf::from(ensure_cache_dir()?).join(format!("input-{pid}.lock")))
}

#[cfg(test)]
pub(super) fn lock_input(pid: i32) -> Result<File, MacosError> {
    lock_path(&input_lock_path(pid)?)
}

#[cfg(test)]
fn lock_path(path: &Path) -> Result<File, MacosError> {
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .map_err(|error| MacosError::Other(format!("input lock: {error}")))?;
    // Darwin LOCK_EX | LOCK_NB. Closing the file releases the kernel-held lock.
    if unsafe { flock(file.as_raw_fd(), 0x02 | 0x04) } != 0 {
        return Err(MacosError::Other(
            "another input action is running for this app; observe before retrying".into(),
        ));
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn input_lock_rejects_overlap_and_releases_on_drop() {
        let dir = tempfile::tempdir().expect("directory");
        let path = dir.path().join("lock");
        let first = lock_path(&path).expect("first action");
        assert!(lock_path(&path).is_err());
        drop(first);
        assert!(lock_path(&path).is_ok());
    }
}
