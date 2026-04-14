use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

use crate::MacosError;
use crate::notes::{AudioAttachment, AudioNote};

use super::{
    apple_to_unix_timestamp, compute_sha256, normalize_media_title, notes_group_container_path,
    open_notes_db, resolve_media_path, since_apple_epoch,
};

pub(crate) fn load_audio_notes(since: DateTime<Utc>) -> Result<Vec<AudioNote>, MacosError> {
    let conn = open_notes_db()?;
    let media_root = notes_group_container_path()?.join("Accounts");

    load_audio_notes_from_conn(&conn, &media_root, since)
}

fn load_audio_notes_from_conn(
    conn: &rusqlite::Connection,
    media_root: &Path,
    since: DateTime<Utc>,
) -> Result<Vec<AudioNote>, MacosError> {
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
            attachment.ZTITLE,
            attachment.ZADDITIONALINDEXABLETEXT,
            attachment.ZSUMMARY,
            attachment.ZOCRSUMMARY,
            attachment.ZDISPLAYTEXT,
            media.ZFILENAME,
            media.ZIDENTIFIER,
            attachment.ZDURATION
        FROM ZICCLOUDSYNCINGOBJECT AS attachment
        JOIN ZICCLOUDSYNCINGOBJECT AS note
            ON attachment.ZNOTE = note.Z_PK
        JOIN ZICCLOUDSYNCINGOBJECT AS media
            ON attachment.ZMEDIA = media.Z_PK
        WHERE attachment.ZTYPEUTI IN ('com.apple.m4a-audio', 'com.microsoft.waveform-audio')
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
        .map_err(|err| MacosError::Other(format!("failed to prepare audio query: {err}")))?;

    let mut rows = stmt
        .query([since_apple_epoch(since)])
        .map_err(|err| MacosError::Other(format!("failed to query audio notes: {err}")))?;

    let mut grouped: HashMap<(i64, Option<String>), HashMap<String, AudioAttachment>> = HashMap::new();
    while let Some(row) = rows
        .next()
        .map_err(|err| MacosError::Other(format!("failed to read audio row: {err}")))?
    {
        let modification_date: f64 = row
            .get(0)
            .map_err(|err| MacosError::Other(format!("failed to decode audio modification date: {err}")))?;
        let note_title: Option<String> = row
            .get(1)
            .map_err(|err| MacosError::Other(format!("failed to decode audio note title: {err}")))?;
        let attachment_title: Option<String> = row
            .get(2)
            .map_err(|err| MacosError::Other(format!("failed to decode audio title: {err}")))?;
        let indexable_text: Option<String> = row
            .get(3)
            .map_err(|err| MacosError::Other(format!("failed to decode audio transcript text: {err}")))?;
        let summary_text: Option<String> = row
            .get(4)
            .map_err(|err| MacosError::Other(format!("failed to decode audio summary: {err}")))?;
        let ocr_summary_text: Option<String> = row
            .get(5)
            .map_err(|err| MacosError::Other(format!("failed to decode audio OCR summary: {err}")))?;
        let display_text: Option<String> = row
            .get(6)
            .map_err(|err| MacosError::Other(format!("failed to decode audio display text: {err}")))?;
        let filename: String = row
            .get(7)
            .map_err(|err| MacosError::Other(format!("failed to decode audio filename: {err}")))?;
        let identifier: String = row
            .get(8)
            .map_err(|err| MacosError::Other(format!("failed to decode audio identifier: {err}")))?;
        let duration_seconds: Option<f64> = row
            .get(9)
            .map_err(|err| MacosError::Other(format!("failed to decode audio duration: {err}")))?;

        let Some(path) = resolve_media_path(media_root, &identifier, &filename) else {
            continue;
        };
        let Some(attachment) = audio_attachment_from_row(
            attachment_title,
            indexable_text,
            summary_text,
            ocr_summary_text,
            display_text,
            filename.clone(),
            path,
            duration_seconds,
        ) else {
            continue;
        };

        let timestamp = apple_to_unix_timestamp(modification_date);
        let note_key = (timestamp, normalize_media_title(note_title));
        let attachments = grouped.entry(note_key).or_default();
        let key = identifier;
        match attachments.remove(&key) {
            Some(existing) => {
                attachments.insert(key, prefer_audio_attachment(existing, attachment));
            }
            None => {
                attachments.insert(key, attachment);
            }
        }
    }

    Ok(grouped
        .into_iter()
        .map(|((timestamp, title), attachments)| AudioNote {
            timestamp,
            title,
            attachments: collapse_duplicate_audio_attachments(attachments.into_values().collect()),
        })
        .collect())
}

fn audio_attachment_from_row(
    title: Option<String>,
    indexable_text: Option<String>,
    summary_text: Option<String>,
    ocr_summary_text: Option<String>,
    display_text: Option<String>,
    filename: String,
    path: PathBuf,
    duration_seconds: Option<f64>,
) -> Option<AudioAttachment> {
    if filename.trim().is_empty() {
        return None;
    }

    let transcript_text = [
        indexable_text,
        summary_text,
        ocr_summary_text,
        display_text,
    ]
    .into_iter()
    .find_map(normalize_optional_text);

    Some(AudioAttachment {
        title: normalize_media_title(title),
        filename,
        sha256: compute_sha256(&path),
        path,
        duration_seconds: duration_seconds.filter(|value| *value > 0.0),
        transcript_text,
    })
}

fn prefer_audio_attachment(existing: AudioAttachment, candidate: AudioAttachment) -> AudioAttachment {
    match (
        existing.transcript_text.is_some(),
        candidate.transcript_text.is_some(),
    ) {
        (false, true) => candidate,
        (true, false) => existing,
        _ => {
            let mut best = existing;
            if best.title.is_none() && candidate.title.is_some() {
                best.title = candidate.title;
            }
            if best.duration_seconds.is_none() {
                best.duration_seconds = candidate.duration_seconds;
            }
            if best.transcript_text.is_none() {
                best.transcript_text = candidate.transcript_text;
            }
            best
        }
    }
}

fn collapse_duplicate_audio_attachments(attachments: Vec<AudioAttachment>) -> Vec<AudioAttachment> {
    let mut deduped: HashMap<String, AudioAttachment> = HashMap::new();

    for attachment in attachments {
        let key = audio_dedup_key(&attachment);
        match deduped.remove(&key) {
            Some(existing) => {
                deduped.insert(key, prefer_audio_attachment(existing, attachment));
            }
            None => {
                deduped.insert(key, attachment);
            }
        }
    }

    let mut attachments = deduped.into_values().collect::<Vec<_>>();
    attachments.sort_by(|left, right| left.filename.cmp(&right.filename));
    attachments
}

fn audio_dedup_key(attachment: &AudioAttachment) -> String {
    match attachment.duration_seconds {
        Some(duration) => format!("{}::{duration:.3}", attachment.filename),
        None => format!("{}::none", attachment.filename),
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use chrono::{TimeZone, Utc};
    use rusqlite::Connection;

    use super::{
        collapse_duplicate_audio_attachments, load_audio_notes_from_conn, prefer_audio_attachment,
    };

    #[test]
    fn load_audio_notes_deduplicates_duplicate_rows_and_prefers_transcript() {
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
                ZTYPEUTI TEXT,
                ZADDITIONALINDEXABLETEXT TEXT,
                ZSUMMARY TEXT,
                ZOCRSUMMARY TEXT,
                ZDISPLAYTEXT TEXT,
                ZDURATION REAL
            );

            INSERT INTO ZICCLOUDSYNCINGOBJECT (Z_PK, ZTITLE1, ZMODIFICATIONDATE1)
            VALUES (1, 'voai-test.wav', 1000.0);

            INSERT INTO ZICCLOUDSYNCINGOBJECT (Z_PK, ZNOTE, ZMEDIA, ZTYPEUTI, ZTITLE, ZMODIFICATIONDATE, ZDURATION)
            VALUES (2, 1, 5, 'com.apple.m4a-audio', '新錄音', 999.0, 3.769);

            INSERT INTO ZICCLOUDSYNCINGOBJECT (Z_PK, ZNOTE, ZMEDIA, ZTYPEUTI, ZTITLE, ZADDITIONALINDEXABLETEXT, ZMODIFICATIONDATE, ZDURATION)
            VALUES (3, 1, 4, 'com.microsoft.waveform-audio', 'voai-test.wav', '老闆你好我是龍工這是語音測試。', 999.0, 3.769);

            INSERT INTO ZICCLOUDSYNCINGOBJECT (Z_PK, ZFILENAME, ZIDENTIFIER)
            VALUES (4, 'voai-test.wav', 'transcript-audio-id');

            INSERT INTO ZICCLOUDSYNCINGOBJECT (Z_PK, ZFILENAME, ZIDENTIFIER)
            VALUES (5, 'voai-test.wav', 'plain-audio-id');
            "#,
        )
        .expect("seed sqlite");

        let media_root = temp.path().join("Accounts");
        let transcript_dir = media_root.join("test-account/Media/transcript-audio-id/child");
        let plain_dir = media_root.join("test-account/Media/plain-audio-id/child");
        fs::create_dir_all(&transcript_dir).expect("create transcript dir");
        fs::create_dir_all(&plain_dir).expect("create plain dir");
        fs::write(transcript_dir.join("voai-test.wav"), b"m4a").expect("write transcript audio");
        fs::write(plain_dir.join("voai-test.wav"), b"wav").expect("write plain audio");

        let since = Utc
            .timestamp_opt(978_307_200 + 900, 0)
            .single()
            .expect("since");
        let notes = load_audio_notes_from_conn(&conn, &media_root, since).expect("load audio notes");

        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].title.as_deref(), Some("voai-test.wav"));
        assert_eq!(notes[0].attachments.len(), 1);
        assert_eq!(notes[0].attachments[0].filename, "voai-test.wav");
        assert_eq!(
            notes[0].attachments[0].title.as_deref(),
            Some("voai-test.wav")
        );
        assert_eq!(notes[0].attachments[0].duration_seconds, Some(3.769));
        assert_eq!(
            notes[0].attachments[0].transcript_text.as_deref(),
            Some("老闆你好我是龍工這是語音測試。")
        );
    }

    #[test]
    fn prefer_audio_attachment_keeps_existing_when_both_have_same_transcript_state() {
        let existing = super::audio_attachment_from_row(
            Some("錄音".into()),
            None,
            None,
            None,
            None,
            "voai-test.wav".into(),
            "/tmp/one.wav".into(),
            Some(3.0),
        )
        .expect("existing");
        let candidate = super::audio_attachment_from_row(
            None,
            None,
            None,
            None,
            None,
            "voai-test.wav".into(),
            "/tmp/two.wav".into(),
            None,
        )
        .expect("candidate");

        let chosen = prefer_audio_attachment(existing, candidate);

        assert_eq!(chosen.title.as_deref(), Some("錄音"));
        assert_eq!(chosen.duration_seconds, Some(3.0));
        assert_eq!(chosen.path, PathBuf::from("/tmp/one.wav"));
    }

    #[test]
    fn collapse_duplicate_audio_attachments_keeps_distinct_same_filename_when_duration_differs() {
        let attachments = vec![
            super::audio_attachment_from_row(
                Some("錄音 A".into()),
                None,
                None,
                None,
                None,
                "same.wav".into(),
                "/tmp/one.wav".into(),
                Some(3.0),
            )
            .expect("attachment a"),
            super::audio_attachment_from_row(
                Some("錄音 B".into()),
                None,
                None,
                None,
                None,
                "same.wav".into(),
                "/tmp/two.wav".into(),
                Some(4.0),
            )
            .expect("attachment b"),
        ];

        let deduped = collapse_duplicate_audio_attachments(attachments);

        assert_eq!(deduped.len(), 2);
    }
}
