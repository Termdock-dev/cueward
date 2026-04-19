mod db;
mod types;

pub use db::{find_shortcut, list_shortcuts, write_shortcut_payload};
pub use types::{ShortcutRecord, ShortcutSelector};

#[cfg(test)]
mod tests;
