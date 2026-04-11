mod safari;
mod notes;
mod messages;
pub mod applescript;
pub mod calendar;
pub mod clipboard;
pub mod send;
pub mod plan;
pub mod ocr;
pub mod quick_notes;
pub mod screenshot;
mod error;

pub use error::MacosError;

use chrono::{DateTime, Utc};
use cueward_core::{Cue, PlatformAdapter};

pub struct MacosAdapter;

impl PlatformAdapter for MacosAdapter {
    type Error = MacosError;

    fn capture_browser_history(
        &self,
        since: DateTime<Utc>,
    ) -> Result<Vec<Cue>, Self::Error> {
        safari::capture(since)
    }

    fn capture_notes(
        &self,
        since: DateTime<Utc>,
    ) -> Result<Vec<Cue>, Self::Error> {
        notes::capture(since)
    }

    fn capture_messages(
        &self,
        since: DateTime<Utc>,
    ) -> Result<Vec<Cue>, Self::Error> {
        messages::capture(since)
    }
}
