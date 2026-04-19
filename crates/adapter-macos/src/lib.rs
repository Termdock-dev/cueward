pub mod applescript;
pub mod bookmarks;
pub mod calendar;
pub mod clipboard;
pub mod doctor;
mod error;
mod messages;
pub mod notes;
pub mod ocr;
pub mod plan;
pub mod quick_notes;
pub mod reddit;
pub mod reminders;
pub mod safari;
mod safari_guard;
pub mod scan_state;
pub mod screenshot;
pub mod shortcuts;
pub mod stickies;
pub mod voice_memos;

pub use error::MacosError;
pub use scan_state::{ScanEnvelope, ScanStatus};

use chrono::{DateTime, Utc};
use cueward_core::{Cue, PlatformAdapter};

pub struct MacosAdapter;

impl PlatformAdapter for MacosAdapter {
    type Error = MacosError;

    fn capture_browser_history(&self, since: DateTime<Utc>) -> Result<Vec<Cue>, Self::Error> {
        safari::capture(since)
    }

    fn capture_notes(&self, since: DateTime<Utc>) -> Result<Vec<Cue>, Self::Error> {
        notes::capture(since)
    }

    fn capture_messages(&self, since: DateTime<Utc>) -> Result<Vec<Cue>, Self::Error> {
        messages::capture(since)
    }
}
