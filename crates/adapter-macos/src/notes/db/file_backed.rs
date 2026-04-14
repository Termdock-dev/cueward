use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

use chrono::{DateTime, Utc};
use cueward_core::AttachmentKind;

use crate::MacosError;
use crate::notes::{FileBackedAttachment, FileBackedNote};

use super::{
    apple_to_unix_timestamp, compute_sha256, normalize_media_title, notes_group_container_path,
    open_notes_db, resolve_media_path, since_apple_epoch,
};

pub(crate) fn load_file_backed_notes(since: DateTime<Utc>) -> Result<Vec<FileBackedNote>, MacosError> {
    let conn = open_notes_db()?;
    let media_root = notes_group_container_path()?.join("Accounts");

    load_file_backed_notes_from_conn(&conn, &media_root, since)
}

fn load_file_backed_notes_from_conn(
    conn: &rusqlite::Connection,
    media_root: &std::path::Path,
    since: DateTime<Utc>,
) -> Result<Vec<FileBackedNote>, MacosError> {

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
            attachment.ZTYPEUTI,
            attachment.ZTITLE,
            media.ZFILENAME,
            media.ZIDENTIFIER
        FROM ZICCLOUDSYNCINGOBJECT AS attachment
        JOIN ZICCLOUDSYNCINGOBJECT AS note
            ON attachment.ZNOTE = note.Z_PK
        JOIN ZICCLOUDSYNCINGOBJECT AS media
            ON attachment.ZMEDIA = media.Z_PK
        WHERE attachment.ZTYPEUTI IN ('com.adobe.pdf', 'public.pdf', 'public.data', 'com.adobe.scan')
          AND media.ZFILENAME IS NOT NULL
          AND media.ZIDENTIFIER IS NOT NULL
          AND COALESCE(
                note.ZMODIFICATIONDATE,
                note.ZMODIFICATIONDATE1,
                attachment.ZMODIFICATIONDATE,
                attachment.ZMODIFICATIONDATE1
              ) > ?
        "#,
        )
        .map_err(|err| MacosError::Other(format!("failed to prepare file-backed query: {err}")))?;

    let mut rows = stmt
        .query([since_apple_epoch(since)])
        .map_err(|err| MacosError::Other(format!("failed to query file-backed notes: {err}")))?;

    let mut grouped: HashMap<(i64, Option<String>), Vec<FileBackedAttachment>> = HashMap::new();
    while let Some(row) = rows
        .next()
        .map_err(|err| MacosError::Other(format!("failed to read file-backed row: {err}")))?
    {
        let modification_date: f64 = row
            .get(0)
            .map_err(|err| MacosError::Other(format!("failed to decode file-backed modification date: {err}")))?;
        let note_title: Option<String> = row
            .get(1)
            .map_err(|err| MacosError::Other(format!("failed to decode file-backed note title: {err}")))?;
        let type_uti: String = row
            .get(2)
            .map_err(|err| MacosError::Other(format!("failed to decode file-backed type: {err}")))?;
        let attachment_title: Option<String> = row
            .get(3)
            .map_err(|err| MacosError::Other(format!("failed to decode file-backed title: {err}")))?;
        let filename: String = row
            .get(4)
            .map_err(|err| MacosError::Other(format!("failed to decode file-backed filename: {err}")))?;
        let identifier: String = row
            .get(5)
            .map_err(|err| MacosError::Other(format!("failed to decode file-backed identifier: {err}")))?;

        let Some(kind) = attachment_kind_from_uti(&type_uti) else {
            continue;
        };
        let path = resolve_media_path(&media_root, &identifier, &filename);
        let Some(attachment) = file_backed_attachment_from_row(kind, attachment_title, filename, path) else {
            continue;
        };

        let timestamp = apple_to_unix_timestamp(modification_date);
        grouped
            .entry((timestamp, normalize_media_title(note_title)))
            .or_default()
            .push(attachment);
    }

    Ok(grouped
        .into_iter()
        .map(|((timestamp, title), attachments)| FileBackedNote {
            timestamp,
            title,
            attachments,
        })
        .collect())
}

fn attachment_kind_from_uti(uti: &str) -> Option<AttachmentKind> {
    match uti {
        "com.adobe.pdf" | "public.pdf" => Some(AttachmentKind::Pdf),
        "public.data" => Some(AttachmentKind::Binary),
        "com.adobe.scan" => Some(AttachmentKind::Scan),
        _ => None,
    }
}

fn file_backed_attachment_from_row(
    kind: AttachmentKind,
    title: Option<String>,
    filename: String,
    path: Option<PathBuf>,
) -> Option<FileBackedAttachment> {
    if filename.trim().is_empty() {
        return None;
    }

    let sha256 = path.as_deref().and_then(compute_sha256);
    let page_count = page_count_for_attachment(&kind, path.as_deref());

    Some(FileBackedAttachment {
        kind,
        title: normalize_media_title(title),
        filename,
        path,
        sha256,
        page_count,
    })
}

fn page_count_for_attachment(kind: &AttachmentKind, path: Option<&std::path::Path>) -> Option<u32> {
    match kind {
        AttachmentKind::Scan => Some(1),
        AttachmentKind::Pdf => path.and_then(pdf_page_count),
        _ => None,
    }
}

fn pdf_page_count(path: &std::path::Path) -> Option<u32> {
    let output = Command::new("mdls")
        .arg("-name")
        .arg("kMDItemNumberOfPages")
        .arg("-raw")
        .arg(path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    parse_mdls_page_count(&String::from_utf8_lossy(&output.stdout))
}

fn parse_mdls_page_count(output: &str) -> Option<u32> {
    let trimmed = output.trim();
    if trimmed.is_empty() || trimmed == "(null)" {
        return None;
    }
    trimmed.parse().ok()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use chrono::{TimeZone, Utc};
    use cueward_core::AttachmentKind;
    use rusqlite::Connection;

    use super::{
        attachment_kind_from_uti, file_backed_attachment_from_row, load_file_backed_notes_from_conn,
        parse_mdls_page_count,
    };

    #[test]
    fn file_backed_attachment_from_row_builds_pdf_attachment() {
        let attachment = file_backed_attachment_from_row(
            AttachmentKind::Pdf,
            Some("SK-INFLUX [V MB]_DS_C0919.pdf".into()),
            "SK-INFLUX [V MB]_DS_C0919.pdf".into(),
            Some(PathBuf::from("/tmp/SK-INFLUX [V MB]_DS_C0919.pdf")),
        )
        .expect("pdf attachment");

        assert_eq!(attachment.kind, AttachmentKind::Pdf);
        assert_eq!(
            attachment.title.as_deref(),
            Some("SK-INFLUX [V MB]_DS_C0919.pdf")
        );
        assert_eq!(attachment.filename, "SK-INFLUX [V MB]_DS_C0919.pdf");
        assert_eq!(attachment.path, Some(PathBuf::from("/tmp/SK-INFLUX [V MB]_DS_C0919.pdf")));
    }

    #[test]
    fn file_backed_attachment_from_row_uses_filename_when_title_missing() {
        let attachment = file_backed_attachment_from_row(
            AttachmentKind::Binary,
            None,
            "blob.bin".into(),
            Some(PathBuf::from("/tmp/blob.bin")),
        )
        .expect("binary attachment");

        assert_eq!(attachment.kind, AttachmentKind::Binary);
        assert_eq!(attachment.title, None);
        assert_eq!(attachment.filename, "blob.bin");
    }

    #[test]
    fn file_backed_attachment_from_row_keeps_missing_path_visible() {
        let attachment = file_backed_attachment_from_row(
            AttachmentKind::Binary,
            Some("blob.bin".into()),
            "blob.bin".into(),
            None,
        )
        .expect("binary attachment");

        assert_eq!(attachment.path, None);
        assert_eq!(attachment.sha256, None);
        assert_eq!(attachment.page_count, None);
    }

    #[test]
    fn attachment_kind_from_uti_maps_pdf_binary_and_scan() {
        assert_eq!(attachment_kind_from_uti("com.adobe.pdf"), Some(AttachmentKind::Pdf));
        assert_eq!(attachment_kind_from_uti("public.pdf"), Some(AttachmentKind::Pdf));
        assert_eq!(attachment_kind_from_uti("public.data"), Some(AttachmentKind::Binary));
        assert_eq!(attachment_kind_from_uti("com.adobe.scan"), Some(AttachmentKind::Scan));
        assert_eq!(attachment_kind_from_uti("public.url"), None);
    }

    #[test]
    fn parse_mdls_page_count_handles_numeric_and_null_output() {
        assert_eq!(parse_mdls_page_count("5\n"), Some(5));
        assert_eq!(parse_mdls_page_count("(null)\n"), None);
        assert_eq!(parse_mdls_page_count(""), None);
    }

    #[test]
    fn load_file_backed_notes_includes_public_pdf_rows() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db_path = temp.path().join("NoteStore.sqlite");
        let conn = Connection::open(&db_path).expect("open sqlite");
        conn.execute_batch(
            r#"
            CREATE TABLE ZICCLOUDSYNCINGOBJECT (
                Z_PK INTEGER PRIMARY KEY,
                ZMODIFICATIONDATE REAL,
                ZMODIFICATIONDATE1 REAL,
                ZTITLE TEXT,
                ZTITLE1 TEXT,
                ZMEDIA INTEGER,
                ZFILENAME TEXT,
                ZIDENTIFIER TEXT,
                ZNOTE INTEGER,
                ZTYPEUTI TEXT
            );

            INSERT INTO ZICCLOUDSYNCINGOBJECT (Z_PK, ZTITLE, ZMODIFICATIONDATE1)
            VALUES (1, 'pdf note', 1000.0);

            INSERT INTO ZICCLOUDSYNCINGOBJECT (Z_PK, ZNOTE, ZMEDIA, ZTYPEUTI, ZTITLE, ZMODIFICATIONDATE)
            VALUES (2, 1, 3, 'public.pdf', 'document.pdf', 999.0);

            INSERT INTO ZICCLOUDSYNCINGOBJECT (Z_PK, ZFILENAME, ZIDENTIFIER)
            VALUES (3, 'document.pdf', 'attachment-id');
            "#,
        )
        .expect("seed sqlite");

        let media_root = temp.path().join("Accounts");
        let media_dir = media_root.join("test-account/Media/attachment-id/child");
        fs::create_dir_all(&media_dir).expect("create media dir");
        fs::write(media_dir.join("document.pdf"), b"pdf").expect("write pdf");

        let since = Utc
            .timestamp_opt(978_307_200 + 900, 0)
            .single()
            .expect("since");
        let notes =
            load_file_backed_notes_from_conn(&conn, &media_root, since).expect("load file-backed notes");

        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].title.as_deref(), Some("pdf note"));
        assert_eq!(notes[0].attachments.len(), 1);
        assert_eq!(notes[0].attachments[0].kind, AttachmentKind::Pdf);
        assert!(notes[0].attachments[0].sha256.is_some());
    }

    #[test]
    fn load_file_backed_notes_includes_scan_rows() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db_path = temp.path().join("NoteStore.sqlite");
        let conn = Connection::open(&db_path).expect("open sqlite");
        conn.execute_batch(
            r#"
            CREATE TABLE ZICCLOUDSYNCINGOBJECT (
                Z_PK INTEGER PRIMARY KEY,
                ZMODIFICATIONDATE REAL,
                ZMODIFICATIONDATE1 REAL,
                ZTITLE TEXT,
                ZTITLE1 TEXT,
                ZMEDIA INTEGER,
                ZFILENAME TEXT,
                ZIDENTIFIER TEXT,
                ZNOTE INTEGER,
                ZTYPEUTI TEXT
            );

            INSERT INTO ZICCLOUDSYNCINGOBJECT (Z_PK, ZTITLE1, ZMODIFICATIONDATE1)
            VALUES (1, 'scan note', 1000.0);

            INSERT INTO ZICCLOUDSYNCINGOBJECT (Z_PK, ZNOTE, ZMEDIA, ZTYPEUTI, ZTITLE, ZMODIFICATIONDATE)
            VALUES (2, 1, 3, 'com.adobe.scan', 'scan.heic', 999.0);

            INSERT INTO ZICCLOUDSYNCINGOBJECT (Z_PK, ZFILENAME, ZIDENTIFIER)
            VALUES (3, 'scan.heic', 'scan-id');
            "#,
        )
        .expect("seed sqlite");

        let media_root = temp.path().join("Accounts");
        let media_dir = media_root.join("test-account/Media/scan-id/child");
        fs::create_dir_all(&media_dir).expect("create media dir");
        fs::write(media_dir.join("scan.heic"), b"scan").expect("write scan");

        let since = Utc
            .timestamp_opt(978_307_200 + 900, 0)
            .single()
            .expect("since");
        let notes =
            load_file_backed_notes_from_conn(&conn, &media_root, since).expect("load file-backed notes");

        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].title.as_deref(), Some("scan note"));
        assert_eq!(notes[0].attachments.len(), 1);
        assert_eq!(notes[0].attachments[0].kind, AttachmentKind::Scan);
        assert_eq!(notes[0].attachments[0].page_count, Some(1));
        assert!(notes[0].attachments[0].sha256.is_some());
    }
}
