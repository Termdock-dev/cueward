use chrono::{DateTime, Utc};

use crate::Cue;

pub trait PlatformAdapter {
    type Error: std::error::Error;

    fn capture_browser_history(
        &self,
        since: DateTime<Utc>,
    ) -> Result<Vec<Cue>, Self::Error>;

    fn capture_notes(
        &self,
        since: DateTime<Utc>,
    ) -> Result<Vec<Cue>, Self::Error>;

    fn capture_messages(
        &self,
        since: DateTime<Utc>,
    ) -> Result<Vec<Cue>, Self::Error>;
}
