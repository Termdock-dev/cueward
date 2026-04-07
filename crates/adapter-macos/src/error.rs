use std::fmt;

#[derive(Debug)]
pub enum MacosError {
    Sqlite(rusqlite::Error),
    PermissionDenied(String),
    NotImplemented(&'static str),
}

impl fmt::Display for MacosError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(e) => write!(f, "sqlite error: {e}"),
            Self::PermissionDenied(path) => write!(
                f,
                "permission denied: {path}\n\n\
                 Grant Full Disk Access to your terminal app:\n  \
                 System Settings > Privacy & Security > Full Disk Access"
            ),
            Self::NotImplemented(name) => write!(f, "{name}: not yet implemented"),
        }
    }
}

impl std::error::Error for MacosError {}

impl From<rusqlite::Error> for MacosError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sqlite(e)
    }
}
