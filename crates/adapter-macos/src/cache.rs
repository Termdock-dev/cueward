//! Shared screenshot/cache and operation-lock directory.
//! Keep the historical path so existing workers continue to share the same locks.
use crate::MacosError;
use std::fs;

const CACHE_DIR: &str = ".cueward/cache/screenshots";

/// Ensure the existing cache directory without changing its ownership or path.
pub(crate) fn ensure_cache_dir() -> Result<String, MacosError> {
    let home = std::env::var("HOME").map_err(|_| MacosError::Other("HOME not set".into()))?;
    let dir = format!("{home}/{CACHE_DIR}");
    fs::create_dir_all(&dir)
        .map_err(|e| MacosError::Other(format!("failed to create {dir}: {e}")))?;
    Ok(dir)
}
