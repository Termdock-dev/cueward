use std::fmt;

pub(crate) const FULL_DISK_ACCESS_GUIDANCE: &str = "Grant Full Disk Access to your terminal app";
pub(crate) const FULL_DISK_ACCESS_SETTINGS_PATH: &str =
    "System Settings > Privacy & Security > Full Disk Access";

#[derive(Debug)]
pub enum MacosError {
    Sqlite(rusqlite::Error),
    PermissionDenied(String),
    NotImplemented(&'static str),
    Other(String),
}

impl fmt::Display for MacosError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(e) => write!(f, "sqlite error: {e}"),
            Self::PermissionDenied(path) => write!(
                f,
                "permission denied: {path}\n\n\
                 {FULL_DISK_ACCESS_GUIDANCE}:\n  \
                 {FULL_DISK_ACCESS_SETTINGS_PATH}"
            ),
            Self::NotImplemented(name) => write!(f, "{name}: not yet implemented"),
            Self::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for MacosError {}

impl From<rusqlite::Error> for MacosError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sqlite(e)
    }
}
