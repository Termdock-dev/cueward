mod adapter;
mod cue;
pub mod inbox;
pub mod index;
mod shortcuts;
mod state;
pub mod tagger;

pub use adapter::PlatformAdapter;
pub use cue::{AttachmentKind, AttachmentSegment, Cue, CueSource};
pub use index::CueIndex;
pub use shortcuts::{
    ShortcutAction, ShortcutInputPolicy, ShortcutReference, ShortcutSpec, ShortcutSurface,
};
pub use state::{ScanTargetState, State};
pub use tagger::Tagger;
