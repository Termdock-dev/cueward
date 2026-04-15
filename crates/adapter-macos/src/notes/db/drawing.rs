use chrono::{DateTime, Utc};
use rusqlite::Connection;

use crate::MacosError;
use crate::notes::{DrawingAttachment, DrawingNote};

use super::{apple_to_unix_timestamp, normalize_media_title, open_notes_db, since_apple_epoch};

pub(crate) fn load_drawing_notes(since: DateTime<Utc>) -> Result<Vec<DrawingNote>, MacosError> {
    let conn = open_notes_db()?;
    load_drawing_notes_from_conn(&conn, since)
}

fn load_drawing_notes_from_conn(
    conn: &Connection,
    since: DateTime<Utc>,
) -> Result<Vec<DrawingNote>, MacosError> {
    let mut stmt = conn
        .prepare(
            r#"
        SELECT
            COALESCE(
                note.ZMODIFICATIONDATE,
                note.ZMODIFICATIONDATE1,
                attachment.ZMODIFICATIONDATE,
                attachment.ZMODIFICATIONDATE1
            ),
            COALESCE(note.ZTITLE, note.ZTITLE1),
            attachment.ZTITLE
        FROM ZICCLOUDSYNCINGOBJECT AS attachment
        JOIN ZICCLOUDSYNCINGOBJECT AS note
            ON attachment.ZNOTE = note.Z_PK
        WHERE attachment.ZTYPEUTI = 'com.apple.paper'
          AND COALESCE(
                note.ZMODIFICATIONDATE,
                note.ZMODIFICATIONDATE1,
                attachment.ZMODIFICATIONDATE,
                attachment.ZMODIFICATIONDATE1
              ) > ?
        "#,
        )
        .map_err(|err| MacosError::Other(format!("failed to prepare drawing query: {err}")))?;

    let mut rows = stmt
        .query([since_apple_epoch(since)])
        .map_err(|err| MacosError::Other(format!("failed to query drawing notes: {err}")))?;

    let mut grouped: std::collections::HashMap<(i64, Option<String>), Vec<DrawingAttachment>> =
        std::collections::HashMap::new();
    while let Some(row) = rows
        .next()
        .map_err(|err| MacosError::Other(format!("failed to read drawing row: {err}")))?
    {
        let modification_date: f64 = row
            .get(0)
            .map_err(|err| MacosError::Other(format!("failed to decode drawing modification date: {err}")))?;
        let note_title: Option<String> = row
            .get(1)
            .map_err(|err| MacosError::Other(format!("failed to decode drawing note title: {err}")))?;
        let attachment_title: Option<String> = row
            .get(2)
            .map_err(|err| MacosError::Other(format!("failed to decode drawing title: {err}")))?;

        let timestamp = apple_to_unix_timestamp(modification_date);
        grouped
            .entry((timestamp, normalize_media_title(note_title)))
            .or_default()
            .push(DrawingAttachment {
                title: normalize_media_title(attachment_title),
            });
    }

    Ok(grouped
        .into_iter()
        .map(|((timestamp, title), attachments)| DrawingNote {
            timestamp,
            title,
            attachments,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use rusqlite::Connection;

    use super::load_drawing_notes_from_conn;

    #[test]
    fn load_drawing_notes_maps_com_apple_paper_rows() {
        let conn = Connection::open_in_memory().expect("open sqlite");
        conn.execute_batch(
            r#"
            CREATE TABLE ZICCLOUDSYNCINGOBJECT (
                Z_PK INTEGER PRIMARY KEY,
                ZMODIFICATIONDATE REAL,
                ZMODIFICATIONDATE1 REAL,
                ZTITLE TEXT,
                ZTITLE1 TEXT,
                ZMEDIA INTEGER,
                ZNOTE INTEGER,
                ZTYPEUTI TEXT
            );

            INSERT INTO ZICCLOUDSYNCINGOBJECT (Z_PK, ZTITLE, ZMODIFICATIONDATE)
            VALUES (590, '新增備忘錄', 1000.0);

            INSERT INTO ZICCLOUDSYNCINGOBJECT (Z_PK, ZNOTE, ZTYPEUTI, ZMODIFICATIONDATE1)
            VALUES (594, 590, 'com.apple.paper', 1000.0);
            "#,
        )
        .expect("seed sqlite");

        let since = Utc
            .timestamp_opt(978_307_200 + 900, 0)
            .single()
            .expect("since");
        let notes = load_drawing_notes_from_conn(&conn, since).expect("load drawing notes");

        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].title.as_deref(), Some("新增備忘錄"));
        assert_eq!(notes[0].attachments.len(), 1);
        assert_eq!(notes[0].attachments[0].title, None);
    }

    #[test]
    fn load_drawing_notes_ignores_non_drawing_rows() {
        let conn = Connection::open_in_memory().expect("open sqlite");
        conn.execute_batch(
            r#"
            CREATE TABLE ZICCLOUDSYNCINGOBJECT (
                Z_PK INTEGER PRIMARY KEY,
                ZMODIFICATIONDATE REAL,
                ZMODIFICATIONDATE1 REAL,
                ZTITLE TEXT,
                ZTITLE1 TEXT,
                ZMEDIA INTEGER,
                ZNOTE INTEGER,
                ZTYPEUTI TEXT
            );

            INSERT INTO ZICCLOUDSYNCINGOBJECT (Z_PK, ZTITLE, ZMODIFICATIONDATE)
            VALUES (1, 'image note', 1000.0);

            INSERT INTO ZICCLOUDSYNCINGOBJECT (Z_PK, ZNOTE, ZTYPEUTI, ZMODIFICATIONDATE1)
            VALUES (2, 1, 'public.png', 1000.0);
            "#,
        )
        .expect("seed sqlite");

        let since = Utc
            .timestamp_opt(978_307_200 + 900, 0)
            .single()
            .expect("since");
        let notes = load_drawing_notes_from_conn(&conn, since).expect("load drawing notes");

        assert!(notes.is_empty());
    }
}
