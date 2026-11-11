//! Atomic local persistence and process-scoped advisory locks.
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use fs4::FileExt;

static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

/// Resolve the local Cueward data directory without falling back to the working directory.
pub fn data_dir() -> io::Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| io::Error::other("HOME must be set"))?;
    Ok(PathBuf::from(home).join(".cueward"))
}

/// Acquire a nonblocking process lock; closing the returned file releases it, including on crash.
pub fn lock(name: &str) -> io::Result<File> {
    if name.is_empty() || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-') {
        return Err(io::Error::other("invalid data lock name"));
    }
    let dir = data_dir()?;
    fs::create_dir_all(&dir)?;
    lock_file(&dir.join(format!("{name}.lock")), name)
}

fn lock_file(path: &Path, name: &str) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    file.try_lock_exclusive()
        .map_err(|e| io::Error::new(e.kind(), format!("{name} is busy: {e}")))?;
    Ok(file)
}

pub(crate) fn unique_name() -> String {
    format!(
        "{}-{}-{}",
        chrono::Utc::now().format("%Y%m%d-%H%M%S%.9f"),
        std::process::id(),
        NEXT_FILE.fetch_add(1, Ordering::Relaxed)
    )
}

pub(crate) fn write_atomic(path: &Path, bytes: &[u8], replace: bool) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("missing parent"))?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".{}.tmp", unique_name()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        if replace {
            fs::rename(&temporary, path)?;
        } else {
            fs::hard_link(&temporary, path)?;
            fs::remove_file(&temporary)?;
        }
        File::open(parent)?.sync_all()
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owned_storage_lock_excludes_another_writer_and_releases_on_drop() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("test.lock");
        let first = lock_file(&path, "test").unwrap();
        assert!(lock_file(&path, "test").is_err());
        drop(first);
        assert!(lock_file(&path, "test").is_ok());
    }
    #[test]
    fn atomic_publish_refuses_overwrite_and_atomic_replace_preserves_complete_bytes() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("data.json");
        write_atomic(&path, b"original", false).unwrap();
        assert!(write_atomic(&path, b"unwanted", false).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");
        write_atomic(&path, b"replacement", true).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"replacement");
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}
