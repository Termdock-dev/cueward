use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct State {
    /// Last successful capture timestamp per source
    pub watermarks: HashMap<String, DateTime<Utc>>,
}

impl State {
    fn path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_default();
        PathBuf::from(home).join(".cueward/state.json")
    }

    pub fn load() -> Self {
        fs::read_to_string(Self::path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)
    }

    pub fn watermark(&self, source: &str) -> Option<DateTime<Utc>> {
        self.watermarks.get(source).copied()
    }

    pub fn set_watermark(&mut self, source: &str, ts: DateTime<Utc>) {
        let entry = self.watermarks.entry(source.to_owned()).or_insert(ts);
        if ts > *entry {
            *entry = ts;
        }
    }
}
