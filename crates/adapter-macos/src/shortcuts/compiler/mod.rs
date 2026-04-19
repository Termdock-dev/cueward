mod decode;
mod encode;

pub use decode::decompile_actions;
pub use encode::{append_action, compile_actions};

use crate::MacosError;

pub fn compiled_action_count(payload: &[u8]) -> Result<usize, MacosError> {
    plist::from_bytes::<Vec<plist::Value>>(payload)
        .map(|actions| actions.len())
        .map_err(|error| MacosError::Other(format!("failed to count compiled shortcut actions: {error}")))
}
