use std::path::{Path, PathBuf};

use chrono::{DateTime, Local, Utc};
use rusqlite::{Connection, OpenFlags, Row};
use serde::Serialize;

use crate::MacosError;

const APPLE_EPOCH_OFFSET: f64 = 978_307_200.0;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct VoiceMemoItem {
    pub id: String,
    pub title: String,
    pub duration_seconds: f64,
    pub timestamp: String,
    pub path: Option<String>,
}

fn voice_memos_root_dir() -> Result<PathBuf, MacosError> {
    let home = std::env::var("HOME")
        .map_err(|_| MacosError::Other("HOME environment variable must be set".into()))?;
    Ok(PathBuf::from(home).join("Library/Group Containers/group.com.apple.VoiceMemos.shared"))
}

fn recordings_dir(root: &Path) -> PathBuf {
    root.join("Recordings")
}

fn voice_memos_db_path(root: &Path) -> PathBuf {
    recordings_dir(root).join("CloudRecordings.db")
}

fn apple_to_rfc3339(value: f64) -> Result<String, MacosError> {
    let unix = value + APPLE_EPOCH_OFFSET;
    let seconds = unix.floor() as i64;
    let nanos = ((unix.fract() * 1_000_000_000.0).round() as u32).min(999_999_999);
    let utc = DateTime::<Utc>::from_timestamp(seconds, nanos)
        .ok_or_else(|| MacosError::Other(format!("invalid voice memo timestamp: {value}")))?;
    Ok(utc.with_timezone(&Local).to_rfc3339())
}

fn preferred_title(custom_label: Option<String>, path: Option<String>, id: &str) -> String {
    custom_label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            path.as_deref().and_then(|path| {
                Path::new(path)
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            })
        })
        .unwrap_or_else(|| id.to_string())
}

fn resolve_recording_path(recordings_root: &Path, relative_path: Option<&str>) -> Option<PathBuf> {
    let relative_path = relative_path?.trim();
    if relative_path.is_empty() {
        return None;
    }

    let relative = Path::new(relative_path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return None;
    }

    let path = recordings_root.join(relative_path);
    if !path.exists() {
        return None;
    }

    let canonical_path = path.canonicalize().ok()?;
    let canonical_root = recordings_root.canonicalize().ok()?;
    if canonical_path.starts_with(&canonical_root) {
        Some(canonical_path)
    } else {
        None
    }
}

fn voice_memo_from_row(row: &Row<'_>, recordings_root: &Path) -> rusqlite::Result<VoiceMemoItem> {
    let date: f64 = row.get(0)?;
    let duration_seconds: f64 = row.get(1)?;
    let custom_label: Option<String> = row.get(2)?;
    let relative_path: Option<String> = row.get(3)?;
    let id: String = row.get(4)?;

    let path = resolve_recording_path(recordings_root, relative_path.as_deref())
        .map(|path| path.display().to_string());
    let title = preferred_title(custom_label, relative_path, &id);
    let timestamp = apple_to_rfc3339(date)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Real,
            Box::new(err),
        ))?;

    Ok(VoiceMemoItem {
        id,
        title,
        duration_seconds,
        timestamp,
        path,
    })
}

fn load_voice_memos_from_conn(
    conn: &Connection,
    recordings_root: &Path,
) -> Result<Vec<VoiceMemoItem>, MacosError> {
    let mut stmt = conn
        .prepare(
            r#"
        SELECT ZDATE, ZDURATION, ZCUSTOMLABEL, ZPATH, ZUNIQUEID
        FROM ZCLOUDRECORDING
        WHERE ZUNIQUEID IS NOT NULL
        ORDER BY ZDATE DESC
        "#,
        )
        .map_err(|err| MacosError::Other(format!("failed to prepare voice memos query: {err}")))?;

    let rows = stmt
        .query_map([], |row| voice_memo_from_row(row, recordings_root))
        .map_err(|err| MacosError::Other(format!("failed to query voice memos: {err}")))?;

    let mut items = Vec::new();
    for row in rows {
        match row {
            Ok(item) => items.push(item),
            Err(err) => eprintln!("warning: failed to read voice memo row: {err}"),
        }
    }
    Ok(items)
}

fn read_voice_memo_from_conn(
    conn: &Connection,
    recordings_root: &Path,
    id: &str,
) -> Result<VoiceMemoItem, MacosError> {
    let mut stmt = conn
        .prepare(
            r#"
        SELECT ZDATE, ZDURATION, ZCUSTOMLABEL, ZPATH, ZUNIQUEID
        FROM ZCLOUDRECORDING
        WHERE ZUNIQUEID = ?
        LIMIT 1
        "#,
        )
        .map_err(|err| MacosError::Other(format!("failed to prepare voice memo read query: {err}")))?;

    stmt.query_row([id], |row| voice_memo_from_row(row, recordings_root))
        .map_err(|err| match err {
            rusqlite::Error::QueryReturnedNoRows => {
                MacosError::Other(format!("voice memo not found: {id}"))
            }
            other => MacosError::Other(format!("failed to read voice memo row: {other}")),
        })
}

/// List all voice memos.
pub fn list_voice_memos() -> Result<Vec<VoiceMemoItem>, MacosError> {
    let root = voice_memos_root_dir()?;
    let db_path = voice_memos_db_path(&root);
    if !db_path.exists() {
        return Err(MacosError::Other("voice memos db not found".into()));
    }
    let conn = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|err| MacosError::Other(format!("failed to open voice memos db: {err}")))?;

    load_voice_memos_from_conn(&conn, &recordings_dir(&root))
}

/// Read one voice memo by id.
pub fn read_voice_memo(id: &str) -> Result<VoiceMemoItem, MacosError> {
    let root = voice_memos_root_dir()?;
    let db_path = voice_memos_db_path(&root);
    if !db_path.exists() {
        return Err(MacosError::Other("voice memos db not found".into()));
    }
    let conn = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|err| MacosError::Other(format!("failed to open voice memos db: {err}")))?;

    read_voice_memo_from_conn(&conn, &recordings_dir(&root), id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed_db(conn: &Connection) {
        conn.execute_batch(
            r#"
            CREATE TABLE ZCLOUDRECORDING (
                Z_PK INTEGER PRIMARY KEY,
                ZDATE REAL,
                ZDURATION REAL,
                ZCUSTOMLABEL TEXT,
                ZPATH TEXT,
                ZUNIQUEID TEXT
            );

            INSERT INTO ZCLOUDRECORDING (Z_PK, ZDATE, ZDURATION, ZCUSTOMLABEL, ZPATH, ZUNIQUEID)
            VALUES
                (1, 766730112.887197, 1.5, '會議紀錄', '20250419 123512-F45D4751.m4a', 'F45D4751-183C-4032-99F7-F1FE1F541BA2'),
                (2, 766126134.361349, 2.4291875, NULL, '20250412 124854-B06D1046.m4a', 'B06D1046-98DB-4C06-9DF1-F633F19B57D6'),
                (3, 585199093.505447, 2430.01469387755, NULL, NULL, '4C630719-98EA-4386-88ED-D3BD631BE50B');
            "#,
        )
        .expect("seed db");
    }

    #[test]
    fn voice_memo_row_prefers_custom_label_then_path_then_id() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db_path = temp.path().join("CloudRecordings.db");
        let recordings_dir = temp.path().join("Recordings");
        std::fs::create_dir_all(&recordings_dir).expect("create recordings dir");
        std::fs::write(recordings_dir.join("20250419 123512-F45D4751.m4a"), b"m4a").expect("write m4a");
        std::fs::write(recordings_dir.join("20250412 124854-B06D1046.m4a"), b"m4a").expect("write m4a");
        let conn = Connection::open(&db_path).expect("open sqlite");
        seed_db(&conn);

        let items = load_voice_memos_from_conn(&conn, &recordings_dir).expect("load");

        assert_eq!(items.len(), 3);
        assert_eq!(items[0].title, "會議紀錄");
        assert_eq!(items[1].title, "20250412 124854-B06D1046");
        assert_eq!(items[2].title, "4C630719-98EA-4386-88ED-D3BD631BE50B");
    }

    #[test]
    fn voice_memo_read_by_id_returns_single_item() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db_path = temp.path().join("CloudRecordings.db");
        let recordings_dir = temp.path().join("Recordings");
        std::fs::create_dir_all(&recordings_dir).expect("create recordings dir");
        std::fs::write(recordings_dir.join("20250419 123512-F45D4751.m4a"), b"m4a").expect("write m4a");
        let conn = Connection::open(&db_path).expect("open sqlite");
        seed_db(&conn);

        let item = read_voice_memo_from_conn(
            &conn,
            &recordings_dir,
            "F45D4751-183C-4032-99F7-F1FE1F541BA2",
        )
        .expect("read");

        assert_eq!(item.id, "F45D4751-183C-4032-99F7-F1FE1F541BA2");
        assert_eq!(item.title, "會議紀錄");
        assert_eq!(item.duration_seconds, 1.5);
        assert!(item.path.is_some());
    }

    #[test]
    fn voice_memo_allows_missing_file_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db_path = temp.path().join("CloudRecordings.db");
        let recordings_dir = temp.path().join("Recordings");
        std::fs::create_dir_all(&recordings_dir).expect("create recordings dir");
        let conn = Connection::open(&db_path).expect("open sqlite");
        seed_db(&conn);

        let items = load_voice_memos_from_conn(&conn, &recordings_dir).expect("load");
        let item = items
            .into_iter()
            .find(|item| item.id == "4C630719-98EA-4386-88ED-D3BD631BE50B")
            .expect("item");

        assert_eq!(item.path, None);
    }

    #[test]
    fn voice_memo_rejects_traversal_like_paths() {
        let recordings_dir = PathBuf::from("/tmp/recordings");

        let resolved = resolve_recording_path(&recordings_dir, Some("../../etc/passwd"));

        assert_eq!(resolved, None);
    }

    #[test]
    fn voice_memo_list_skips_bad_rows() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db_path = temp.path().join("CloudRecordings.db");
        let recordings_dir = temp.path().join("Recordings");
        std::fs::create_dir_all(&recordings_dir).expect("create recordings dir");
        std::fs::write(recordings_dir.join("20250419 123512-F45D4751.m4a"), b"m4a")
            .expect("write m4a");
        let conn = Connection::open(&db_path).expect("open sqlite");
        conn.execute_batch(
            r#"
            CREATE TABLE ZCLOUDRECORDING (
                Z_PK INTEGER PRIMARY KEY,
                ZDATE REAL,
                ZDURATION REAL,
                ZCUSTOMLABEL TEXT,
                ZPATH TEXT,
                ZUNIQUEID TEXT
            );

            INSERT INTO ZCLOUDRECORDING (Z_PK, ZDATE, ZDURATION, ZCUSTOMLABEL, ZPATH, ZUNIQUEID)
            VALUES
                (1, NULL, 1.5, '壞資料', '20250419 123512-F45D4751.m4a', 'BAD'),
                (2, 766730112.887197, 1.5, '會議紀錄', '20250419 123512-F45D4751.m4a', 'GOOD');
            "#,
        )
        .expect("seed db");

        let items = load_voice_memos_from_conn(&conn, &recordings_dir).expect("load");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "GOOD");
    }
}
