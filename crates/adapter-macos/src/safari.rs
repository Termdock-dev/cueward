use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, TimeZone, Utc};
use rusqlite::Connection;

use cueward_core::{Cue, CueSource};

use crate::MacosError;

/// Core Data epoch: 2001-01-01 00:00:00 UTC
const CORE_DATA_EPOCH: i64 = 978_307_200;

fn history_db_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join("Library/Safari/History.db")
}

fn to_core_data_timestamp(dt: DateTime<Utc>) -> f64 {
    (dt.timestamp() - CORE_DATA_EPOCH) as f64
}

fn from_core_data_timestamp(ts: f64) -> DateTime<Utc> {
    Utc.timestamp_opt(ts as i64 + CORE_DATA_EPOCH, 0)
        .single()
        .unwrap_or_default()
}

pub fn capture(since: DateTime<Utc>) -> Result<Vec<Cue>, MacosError> {
    let db_path = history_db_path();

    if !db_path.exists() {
        return Err(MacosError::PermissionDenied(
            db_path.to_string_lossy().into_owned(),
        ));
    }

    let conn = Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| {
        if e.to_string().contains("unable to open") {
            MacosError::PermissionDenied(db_path.to_string_lossy().into_owned())
        } else {
            MacosError::Sqlite(e)
        }
    })?;

    let since_ts = to_core_data_timestamp(since);

    let mut stmt = conn.prepare(
        "SELECT v.visit_time, v.title, i.url \
         FROM history_visits v \
         JOIN history_items i ON v.history_item = i.id \
         WHERE v.visit_time > ?1 \
         ORDER BY v.visit_time DESC",
    )?;

    let cues = stmt
        .query_map([since_ts], |row| {
            let visit_time: f64 = row.get(0)?;
            let title: Option<String> = row.get(1)?;
            let url: String = row.get(2)?;
            Ok((visit_time, title, url))
        })?
        .filter_map(|r| r.ok())
        .map(|(visit_time, title, url)| Cue {
            source: CueSource::Safari,
            timestamp: from_core_data_timestamp(visit_time),
            content: title.clone().unwrap_or_default(),
            url: Some(url),
            title,
            tags: Vec::new(),
            metadata: HashMap::new(),
        })
        .collect();

    Ok(cues)
}
