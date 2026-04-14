use std::collections::HashMap;

use chrono::{DateTime, Utc};
use urlencoding::decode;

use crate::MacosError;
use crate::notes::{MapAttachment, MapNote, WebPreviewAttachment, WebPreviewNote};

use super::{apple_to_unix_timestamp, normalize_media_title, open_notes_db, since_apple_epoch};

pub(crate) fn load_web_preview_notes(since: DateTime<Utc>) -> Result<Vec<WebPreviewNote>, MacosError> {
    let conn = open_notes_db()?;

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
            note.ZTITLE1,
            attachment.ZTITLE,
            attachment.ZURLSTRING
        FROM ZICCLOUDSYNCINGOBJECT AS attachment
        JOIN ZICCLOUDSYNCINGOBJECT AS note
            ON attachment.ZNOTE = note.Z_PK
        WHERE attachment.ZTYPEUTI = 'public.url'
          AND attachment.ZURLSTRING IS NOT NULL
          AND attachment.ZURLSTRING != ''
          AND COALESCE(
                note.ZMODIFICATIONDATE,
                note.ZMODIFICATIONDATE1,
                attachment.ZMODIFICATIONDATE,
                attachment.ZMODIFICATIONDATE1
              ) > ?
        "#,
        )
        .map_err(|err| MacosError::Other(format!("failed to prepare web preview query: {err}")))?;

    let mut rows = stmt
        .query([since_apple_epoch(since)])
        .map_err(|err| MacosError::Other(format!("failed to query web previews: {err}")))?;

    let mut grouped: HashMap<(i64, Option<String>), Vec<WebPreviewAttachment>> = HashMap::new();
    while let Some(row) = rows
        .next()
        .map_err(|err| MacosError::Other(format!("failed to read web preview row: {err}")))?
    {
        let modification_date: f64 = row.get(0).map_err(|err| {
            MacosError::Other(format!("failed to decode web preview modification date: {err}"))
        })?;
        let note_title: Option<String> = row
            .get(1)
            .map_err(|err| MacosError::Other(format!("failed to decode note title: {err}")))?;
        let attachment_title: Option<String> = row.get(2).map_err(|err| {
            MacosError::Other(format!("failed to decode web preview title: {err}"))
        })?;
        let url: String = row.get(3).map_err(|err| {
            MacosError::Other(format!("failed to decode web preview url: {err}"))
        })?;

        let normalized_note_title = normalize_media_title(note_title);
        if is_apple_maps_url(&url)
            && map_attachment_from_row(attachment_title.clone(), normalized_note_title.as_deref(), url.clone())
                .is_some()
        {
            continue;
        }

        let Some(attachment) =
            web_preview_attachment_from_row(attachment_title, normalized_note_title.as_deref(), url)
        else {
            continue;
        };

        let timestamp = apple_to_unix_timestamp(modification_date);
        grouped
            .entry((timestamp, normalized_note_title))
            .or_default()
            .push(attachment);
    }

    Ok(grouped
        .into_iter()
        .map(|((timestamp, title), attachments)| WebPreviewNote {
            timestamp,
            title,
            attachments,
        })
        .collect())
}

pub(crate) fn load_map_notes(since: DateTime<Utc>) -> Result<Vec<MapNote>, MacosError> {
    let conn = open_notes_db()?;

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
            note.ZTITLE1,
            attachment.ZTITLE,
            attachment.ZURLSTRING
        FROM ZICCLOUDSYNCINGOBJECT AS attachment
        JOIN ZICCLOUDSYNCINGOBJECT AS note
            ON attachment.ZNOTE = note.Z_PK
        WHERE attachment.ZTYPEUTI = 'public.url'
          AND attachment.ZURLSTRING LIKE '%maps.apple.com/%'
          AND COALESCE(
                note.ZMODIFICATIONDATE,
                note.ZMODIFICATIONDATE1,
                attachment.ZMODIFICATIONDATE,
                attachment.ZMODIFICATIONDATE1
              ) > ?
        "#,
        )
        .map_err(|err| MacosError::Other(format!("failed to prepare map query: {err}")))?;

    let mut rows = stmt
        .query([since_apple_epoch(since)])
        .map_err(|err| MacosError::Other(format!("failed to query maps: {err}")))?;

    let mut grouped: HashMap<(i64, Option<String>), Vec<MapAttachment>> = HashMap::new();
    while let Some(row) = rows
        .next()
        .map_err(|err| MacosError::Other(format!("failed to read map row: {err}")))?
    {
        let modification_date: f64 = row
            .get(0)
            .map_err(|err| MacosError::Other(format!("failed to decode map modification date: {err}")))?;
        let note_title: Option<String> = row
            .get(1)
            .map_err(|err| MacosError::Other(format!("failed to decode map note title: {err}")))?;
        let attachment_title: Option<String> = row
            .get(2)
            .map_err(|err| MacosError::Other(format!("failed to decode map title: {err}")))?;
        let url: String = row
            .get(3)
            .map_err(|err| MacosError::Other(format!("failed to decode map url: {err}")))?;

        let normalized_note_title = normalize_media_title(note_title);
        let Some(attachment) =
            map_attachment_from_row(attachment_title, normalized_note_title.as_deref(), url)
        else {
            continue;
        };

        let timestamp = apple_to_unix_timestamp(modification_date);
        grouped
            .entry((timestamp, normalized_note_title))
            .or_default()
            .push(attachment);
    }

    Ok(grouped
        .into_iter()
        .map(|((timestamp, title), attachments)| MapNote {
            timestamp,
            title,
            attachments,
        })
        .collect())
}

fn preferred_web_preview_title(
    attachment_title: Option<String>,
    note_title: Option<&str>,
    url: &str,
) -> Option<String> {
    normalize_media_title(attachment_title)
        .or_else(|| normalize_media_title(note_title.map(str::to_string)))
        .or_else(|| {
            let trimmed = url.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
}

fn web_preview_attachment_from_row(
    attachment_title: Option<String>,
    note_title: Option<&str>,
    url: String,
) -> Option<WebPreviewAttachment> {
    let url = url.trim().to_string();
    if url.is_empty() {
        return None;
    }

    Some(WebPreviewAttachment {
        title: preferred_web_preview_title(attachment_title, note_title, &url),
        url,
    })
}

fn preferred_map_title(
    attachment_title: Option<String>,
    note_title: Option<&str>,
    url_name: Option<&str>,
) -> Option<String> {
    normalize_media_title(attachment_title)
        .or_else(|| normalize_media_title(url_name.map(str::to_string)))
        .or_else(|| normalize_media_title(note_title.map(str::to_string)))
}

fn is_apple_maps_url(url: &str) -> bool {
    url.contains("maps.apple.com/")
}

fn parse_map_coordinate(url: &str) -> Option<(f64, f64)> {
    let query = url.split('#').next()?.split('?').nth(1)?;
    let coordinate = query
        .split('&')
        .find_map(|pair| pair.strip_prefix("coordinate="))?;
    let mut parts = coordinate.split(',');
    let latitude = parts.next()?.parse::<f64>().ok()?;
    let longitude = parts.next()?.parse::<f64>().ok()?;

    if !(-90.0..=90.0).contains(&latitude) || !(-180.0..=180.0).contains(&longitude) {
        return None;
    }

    Some((latitude, longitude))
}

fn parse_map_name(url: &str) -> Option<String> {
    let query = url.split('#').next()?.split('?').nth(1)?;
    let encoded = query
        .split('&')
        .find_map(|pair| pair.strip_prefix("name="))?;
    let decoded = decode(encoded).ok()?;
    let trimmed = decoded.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn map_attachment_from_row(
    attachment_title: Option<String>,
    note_title: Option<&str>,
    url: String,
) -> Option<MapAttachment> {
    if !is_apple_maps_url(&url) {
        return None;
    }

    let (latitude, longitude) = parse_map_coordinate(&url)?;
    let title = preferred_map_title(attachment_title, note_title, parse_map_name(&url).as_deref());

    Some(MapAttachment {
        title,
        url: Some(url),
        latitude,
        longitude,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        map_attachment_from_row, preferred_map_title, preferred_web_preview_title,
        web_preview_attachment_from_row,
    };

    #[test]
    fn preferred_web_preview_title_prefers_attachment_then_note_then_url() {
        assert_eq!(
            preferred_web_preview_title(
                Some("Cursor Docs".into()),
                Some("Working with Context"),
                "https://docs.cursor.com/guides/working-with-context",
            ),
            Some("Cursor Docs".into())
        );
        assert_eq!(
            preferred_web_preview_title(
                None,
                Some("Working with Context"),
                "https://docs.cursor.com/guides/working-with-context",
            ),
            Some("Working with Context".into())
        );
        assert_eq!(
            preferred_web_preview_title(
                None,
                None,
                "https://docs.cursor.com/guides/working-with-context",
            ),
            Some("https://docs.cursor.com/guides/working-with-context".into())
        );
    }

    #[test]
    fn web_preview_attachment_from_row_ignores_empty_url() {
        assert_eq!(
            web_preview_attachment_from_row(
                Some("Cursor Docs".into()),
                Some("Working with Context"),
                "   ".into(),
            ),
            None
        );
    }

    #[test]
    fn web_preview_attachment_from_row_builds_structured_attachment() {
        let attachment = web_preview_attachment_from_row(
            Some("Cursor Docs".into()),
            Some("Working with Context"),
            "https://docs.cursor.com/guides/working-with-context".into(),
        )
        .expect("attachment");

        assert_eq!(attachment.title.as_deref(), Some("Cursor Docs"));
        assert_eq!(
            attachment.url,
            "https://docs.cursor.com/guides/working-with-context"
        );
    }

    #[test]
    fn preferred_map_title_prefers_attachment_then_url_name_then_note() {
        assert_eq!(
            preferred_map_title(
                Some("屏東縣立棒球場".into()),
                Some("備忘錄標題"),
                Some("地圖上的名稱")
            ),
            Some("屏東縣立棒球場".into())
        );
        assert_eq!(
            preferred_map_title(None, Some("備忘錄標題"), Some("地圖上的名稱")),
            Some("地圖上的名稱".into())
        );
        assert_eq!(
            preferred_map_title(None, Some("備忘錄標題"), None),
            Some("備忘錄標題".into())
        );
    }

    #[test]
    fn map_attachment_from_row_parses_coordinate_and_name() {
        let attachment = map_attachment_from_row(
            Some("屏東縣立棒球場".into()),
            Some("新增備忘錄"),
            "https://maps.apple.com/place?address=900044%E5%8F%B0%E7%81%A3%E5%B1%8F%E6%9D%B1%E7%B8%A3%E5%B1%8F%E6%9D%B1%E5%B8%82%E6%A3%92%E7%90%83%E8%B7%AF1%E8%99%9F&coordinate=22.657349,120.485956&name=%E5%B1%8F%E6%9D%B1%E7%B8%A3%E7%AB%8B%E6%A3%92%E7%90%83%E5%A0%B4&place-id=I3A6B061B285192F6".into(),
        )
        .expect("map attachment");

        assert_eq!(attachment.title.as_deref(), Some("屏東縣立棒球場"));
        assert_eq!(attachment.url.as_deref(), Some("https://maps.apple.com/place?address=900044%E5%8F%B0%E7%81%A3%E5%B1%8F%E6%9D%B1%E7%B8%A3%E5%B1%8F%E6%9D%B1%E5%B8%82%E6%A3%92%E7%90%83%E8%B7%AF1%E8%99%9F&coordinate=22.657349,120.485956&name=%E5%B1%8F%E6%9D%B1%E7%B8%A3%E7%AB%8B%E6%A3%92%E7%90%83%E5%A0%B4&place-id=I3A6B061B285192F6"));
        assert_eq!(attachment.latitude, 22.657349);
        assert_eq!(attachment.longitude, 120.485956);
    }

    #[test]
    fn map_attachment_from_row_ignores_non_map_urls() {
        assert_eq!(
            map_attachment_from_row(
                Some("Cursor Docs".into()),
                Some("Working with Context"),
                "https://docs.cursor.com/guides/working-with-context".into(),
            ),
            None
        );
    }

    #[test]
    fn map_attachment_from_row_rejects_out_of_range_coordinates() {
        assert_eq!(
            map_attachment_from_row(
                Some("Bad Place".into()),
                Some("新增備忘錄"),
                "https://maps.apple.com/place?coordinate=123.0,456.0&name=Bad".into(),
            ),
            None
        );
    }

    #[test]
    fn map_attachment_from_row_handles_url_fragment() {
        let attachment = map_attachment_from_row(
            Some("屏東縣立棒球場".into()),
            Some("新增備忘錄"),
            "https://maps.apple.com/place?coordinate=22.657349,120.485956&name=%E5%B1%8F%E6%9D%B1%E7%B8%A3%E7%AB%8B%E6%A3%92%E7%90%83%E5%A0%B4#label".into(),
        )
        .expect("map attachment with fragment");

        assert_eq!(attachment.latitude, 22.657349);
        assert_eq!(attachment.longitude, 120.485956);
        assert_eq!(attachment.title.as_deref(), Some("屏東縣立棒球場"));
    }
}
