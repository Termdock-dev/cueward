mod file_backed;
mod web_preview;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OpenFlags};

use crate::MacosError;

use super::{APPLE_EPOCH_OFFSET, MAX_MEDIA_SEARCH_DEPTH, MediaAttachment, MediaNote, home_dir};

pub(super) use file_backed::load_file_backed_notes;
pub(super) use web_preview::{load_map_notes, load_web_preview_notes};

pub(super) fn load_media_notes(since: DateTime<Utc>) -> Result<Vec<MediaNote>, MacosError> {
    let note_store = notes_group_container_path()?.join("NoteStore.sqlite");
    let media_root = notes_group_container_path()?.join("Accounts");
    let conn = Connection::open_with_flags(
        note_store,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|err| MacosError::Other(format!("failed to open NoteStore.sqlite: {err}")))?;

    load_media_notes_from_conn(&conn, &media_root, since)
}

fn load_media_notes_from_conn(
    conn: &Connection,
    media_root: &Path,
    since: DateTime<Utc>,
) -> Result<Vec<MediaNote>, MacosError> {

    let mut stmt = conn
        .prepare(
            r#"
        SELECT
            note.ZMODIFICATIONDATE,
            note.ZTITLE,
            media.ZFILENAME,
            media.ZIDENTIFIER
        FROM ZICCLOUDSYNCINGOBJECT AS note
        JOIN ZICCLOUDSYNCINGOBJECT AS media
            ON note.ZMEDIA = media.Z_PK
        LEFT JOIN ZICCLOUDSYNCINGOBJECT AS attachment
            ON attachment.ZMEDIA = media.Z_PK
           AND attachment.ZNOTE = note.Z_PK
        WHERE note.ZMEDIA IS NOT NULL
          AND note.ZMEDIA != 0
          AND (note.ZNOTE IS NULL OR note.ZNOTE = 0)
          AND media.ZFILENAME IS NOT NULL
          AND media.ZIDENTIFIER IS NOT NULL
          AND (
                attachment.ZTYPEUTI IS NULL
                OR attachment.ZTYPEUTI NOT IN ('com.adobe.pdf', 'public.data', 'public.pdf', 'com.adobe.scan')
              )
          AND note.ZMODIFICATIONDATE > ?
        "#,
        )
        .map_err(|err| MacosError::Other(format!("failed to prepare media query: {err}")))?;

    let since_apple_epoch = since.timestamp() as f64 - APPLE_EPOCH_OFFSET;
    let mut rows = stmt
        .query([since_apple_epoch])
        .map_err(|err| MacosError::Other(format!("failed to query note media: {err}")))?;

    let mut grouped: HashMap<(i64, Option<String>), Vec<MediaAttachment>> = HashMap::new();
    while let Some(row) = rows
        .next()
        .map_err(|err| MacosError::Other(format!("failed to read media row: {err}")))?
    {
        let modification_date: f64 = row.get(0).map_err(|err| {
            MacosError::Other(format!("failed to decode modification date: {err}"))
        })?;
        let title: Option<String> = row
            .get(1)
            .map_err(|err| MacosError::Other(format!("failed to decode note title: {err}")))?;
        let filename: String = row.get(2).map_err(|err| {
            MacosError::Other(format!("failed to decode attachment filename: {err}"))
        })?;
        let identifier: String = row.get(3).map_err(|err| {
            MacosError::Other(format!("failed to decode attachment identifier: {err}"))
        })?;

        let Some(path) = resolve_media_path(&media_root, &identifier, &filename) else {
            continue;
        };

        let timestamp = (modification_date + APPLE_EPOCH_OFFSET).round() as i64;
        grouped
            .entry((timestamp, normalize_media_title(title)))
            .or_default()
            .push(MediaAttachment {
                filename,
                path,
                sha256: None,
            });
    }

    Ok(grouped
        .into_iter()
        .map(|((timestamp, title), attachments)| MediaNote {
            timestamp,
            title,
            attachments,
        })
        .collect())
}

pub(super) fn open_notes_db() -> Result<Connection, MacosError> {
    let note_store = notes_group_container_path()?.join("NoteStore.sqlite");
    Connection::open_with_flags(
        note_store,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|err| MacosError::Other(format!("failed to open NoteStore.sqlite: {err}")))
}

pub(super) fn since_apple_epoch(since: DateTime<Utc>) -> f64 {
    since.timestamp() as f64 - APPLE_EPOCH_OFFSET
}

pub(super) fn apple_to_unix_timestamp(apple_date: f64) -> i64 {
    (apple_date + APPLE_EPOCH_OFFSET).round() as i64
}

pub(super) fn normalize_media_title(title: Option<String>) -> Option<String> {
    title.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

pub(super) fn notes_group_container_path() -> Result<PathBuf, MacosError> {
    Ok(home_dir()?.join("Library/Group Containers/group.com.apple.notes"))
}

pub(super) fn resolve_media_path(media_root: &Path, identifier: &str, filename: &str) -> Option<PathBuf> {
    let accounts = fs::read_dir(media_root).ok()?;
    for account in accounts.flatten() {
        let media_dir = account.path().join("Media").join(identifier);
        if !media_dir.is_dir() {
            continue;
        }

        if let Some(path) = find_named_file(&media_dir, filename) {
            return Some(path);
        }
    }

    None
}

fn find_named_file(root: &Path, filename: &str) -> Option<PathBuf> {
    find_named_file_impl(root, filename, 0)
}

fn find_named_file_impl(root: &Path, filename: &str, depth: usize) -> Option<PathBuf> {
    if depth > MAX_MEDIA_SEARCH_DEPTH {
        return None;
    }

    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let file_type = entry.file_type().ok()?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_file() && path.file_name().and_then(|name| name.to_str()) == Some(filename)
        {
            return Some(path);
        }
        if file_type.is_dir() {
            if let Some(found) = find_named_file_impl(&path, filename, depth + 1) {
                return Some(found);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::fs;

    use chrono::{TimeZone, Utc};
    use rusqlite::Connection;

    use super::{apple_to_unix_timestamp, find_named_file, load_media_notes_from_conn};

    #[test]
    fn find_named_file_walks_nested_media_directory() {
        let temp = tempfile::tempdir().expect("tempdir");
        let nested = temp.path().join("a/b");
        fs::create_dir_all(&nested).expect("create nested dirs");
        let target = nested.join("image.png");
        fs::write(&target, b"png").expect("write media file");

        let found = find_named_file(temp.path(), "image.png");

        assert_eq!(found, Some(target));
    }

    #[test]
    fn find_named_file_respects_max_depth() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut current = temp.path().to_path_buf();
        for idx in 0..12 {
            current = current.join(format!("d{idx}"));
        }
        fs::create_dir_all(&current).expect("create nested dirs");
        let target = current.join("image.png");
        fs::write(&target, b"png").expect("write media file");

        let found = find_named_file(temp.path(), "image.png");

        assert_eq!(found, None);
    }

    #[test]
    fn apple_to_unix_timestamp_uses_shared_epoch_offset() {
        assert_eq!(apple_to_unix_timestamp(0.0), 978_307_200);
        assert_eq!(apple_to_unix_timestamp(1.4), 978_307_201);
    }

    #[test]
    fn load_media_notes_ignores_attachment_child_rows() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db_path = temp.path().join("NoteStore.sqlite");
        let conn = Connection::open(&db_path).expect("open sqlite");
        conn.execute_batch(
            r#"
            CREATE TABLE ZICCLOUDSYNCINGOBJECT (
                Z_PK INTEGER PRIMARY KEY,
                ZMODIFICATIONDATE REAL,
                ZTITLE TEXT,
                ZMEDIA INTEGER,
                ZFILENAME TEXT,
                ZIDENTIFIER TEXT,
                ZNOTE INTEGER,
                ZTYPEUTI TEXT
            );

            INSERT INTO ZICCLOUDSYNCINGOBJECT (Z_PK, ZMODIFICATIONDATE, ZTITLE, ZMEDIA, ZNOTE, ZTYPEUTI)
            VALUES (1, 1000.0, 'test', NULL, NULL, NULL);

            INSERT INTO ZICCLOUDSYNCINGOBJECT (Z_PK, ZMODIFICATIONDATE, ZTITLE, ZMEDIA, ZNOTE, ZTYPEUTI)
            VALUES (2, 1000.0, NULL, 3, 1, 'com.adobe.pdf');

            INSERT INTO ZICCLOUDSYNCINGOBJECT (Z_PK, ZMODIFICATIONDATE, ZTITLE, ZMEDIA, ZFILENAME, ZIDENTIFIER)
            VALUES (3, NULL, NULL, NULL, 'doc.pdf', 'attachment-id');
            "#,
        )
        .expect("seed sqlite");

        let media_root = temp.path().join("Accounts");
        let media_dir = media_root.join("test-account/Media/attachment-id/child");
        fs::create_dir_all(&media_dir).expect("create media dir");
        fs::write(media_dir.join("doc.pdf"), b"pdf").expect("write pdf");

        let since = Utc.timestamp_opt(978_307_200 + 900, 0).single().expect("since");
        let notes = load_media_notes_from_conn(&conn, &media_root, since).expect("load media notes");

        assert!(notes.is_empty());
    }
}
