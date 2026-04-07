use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, TimeZone, Utc};
use rusqlite::Connection;

use cueward_core::{Cue, CueSource};

use crate::MacosError;

/// Core Data epoch: 2001-01-01 00:00:00 UTC
const CORE_DATA_EPOCH: i64 = 978_307_200;

/// chat.db stores date as nanoseconds since Core Data epoch
const NANOS_PER_SEC: i64 = 1_000_000_000;

fn chat_db_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join("Library/Messages/chat.db")
}

fn to_chat_timestamp(dt: DateTime<Utc>) -> i64 {
    (dt.timestamp() - CORE_DATA_EPOCH) * NANOS_PER_SEC
}

fn from_chat_timestamp(ns: i64) -> DateTime<Utc> {
    let secs = ns / NANOS_PER_SEC + CORE_DATA_EPOCH;
    Utc.timestamp_opt(secs, 0).single().unwrap_or_default()
}

pub fn capture(since: DateTime<Utc>) -> Result<Vec<Cue>, MacosError> {
    let db_path = chat_db_path();

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

    let since_ts = to_chat_timestamp(since);

    let mut stmt = conn.prepare(
        "SELECT m.date, m.text, m.is_from_me, h.id \
         FROM message m \
         LEFT JOIN handle h ON m.handle_id = h.ROWID \
         WHERE m.text IS NOT NULL AND m.date > ?1 \
         ORDER BY m.date DESC",
    )?;

    let cues = stmt
        .query_map([since_ts], |row| {
            let date: i64 = row.get(0)?;
            let text: String = row.get(1)?;
            let is_from_me: bool = row.get(2)?;
            let sender: Option<String> = row.get(3)?;
            Ok((date, text, is_from_me, sender))
        })?
        .filter_map(|r| r.ok())
        .map(|(date, text, is_from_me, sender)| {
            let mut metadata = HashMap::new();
            if is_from_me {
                metadata.insert("direction".into(), "sent".into());
            } else {
                metadata.insert("direction".into(), "received".into());
            }
            if let Some(s) = sender {
                metadata.insert("sender".into(), s);
            }

            Cue {
                source: CueSource::Messages,
                timestamp: from_chat_timestamp(date),
                content: text,
                url: None,
                title: None,
                tags: Vec::new(),
                metadata,
            }
        })
        .collect();

    Ok(cues)
}
