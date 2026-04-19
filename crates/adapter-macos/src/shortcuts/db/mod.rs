use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::MacosError;

mod payloads;
mod queries;
mod relations;

pub use payloads::{
    encode_input_classes, rename_shortcut_name_by_workflow_id_live,
    update_shortcut_actions_blob_live, update_shortcut_input_classes_live, write_shortcut_payload,
    write_shortcut_payload_live,
};
pub use queries::{
    find_latest_shortcut_after_pk_live, find_shortcut, find_shortcut_live, latest_shortcut_pk_live,
    list_shortcuts, list_shortcuts_live, load_shortcut_input_policy_live, load_shortcut_payload_live,
    load_shortcut_surfaces_live, shortcut_has_relation_live,
};
pub use relations::{
    ensure_shortcut_folder_relation_live, ensure_shortcut_relation_live, sync_shortcut_surfaces_live,
};

#[cfg(test)]
pub use payloads::rename_shortcut_name_by_workflow_id;
#[cfg(test)]
pub use queries::find_latest_shortcut_after_pk;
#[cfg(test)]
pub use queries::load_shortcut_payload;
#[cfg(test)]
pub use relations::ensure_shortcut_folder_relation;
#[cfg(test)]
pub use relations::ensure_shortcut_relation;
#[cfg(test)]
pub use relations::sync_shortcut_surfaces;

fn open_db(db_path: &Path) -> Result<Connection, MacosError> {
    Connection::open(db_path).map_err(MacosError::from)
}

fn default_db_path() -> Result<PathBuf, MacosError> {
    let home = std::env::var("HOME")
        .map_err(|err| MacosError::Other(format!("failed to resolve HOME for Shortcuts db: {err}")))?;
    Ok(PathBuf::from(home).join("Library/Shortcuts/Shortcuts.sqlite"))
}
