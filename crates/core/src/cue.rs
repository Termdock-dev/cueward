use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CueSource {
    Safari,
    Notes,
    Messages,
    Ocr,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentKind {
    Image,
    WebPreview,
    Binary,
    Pdf,
    Scan,
    Audio,
    Map,
    Drawing,
    Unresolved,
}

impl Default for AttachmentKind {
    fn default() -> Self {
        Self::Unresolved
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentSegment {
    pub index: usize,
    #[serde(default)]
    pub kind: AttachmentKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latitude: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub longitude: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ocr_text: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub has_ocr: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cue {
    pub source: CueSource,
    pub timestamp: DateTime<Utc>,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachment_segments: Vec<AttachmentSegment>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::{AttachmentKind, AttachmentSegment};

    #[test]
    fn attachment_segment_serializes_kind_for_image() {
        let segment = AttachmentSegment {
            index: 1,
            kind: AttachmentKind::Image,
            title: None,
            url: None,
            latitude: None,
            longitude: None,
            filename: Some("scan.jpg".into()),
            path: Some("/tmp/scan.jpg".into()),
            sha256: Some("abc123".into()),
            ocr_text: None,
            has_ocr: false,
        };

        let value = serde_json::to_value(segment).expect("serialize segment");

        assert_eq!(value["kind"], "image");
        assert_eq!(value["filename"], "scan.jpg");
        assert_eq!(value["path"], "/tmp/scan.jpg");
    }

    #[test]
    fn attachment_segment_deserializes_legacy_json_without_kind() {
        let segment: AttachmentSegment = serde_json::from_str(
            r#"{
                "index": 1,
                "filename": "scan.jpg",
                "path": "/tmp/scan.jpg",
                "sha256": "abc123",
                "has_ocr": false
            }"#,
        )
        .expect("decode legacy segment");

        assert!(matches!(segment.kind, AttachmentKind::Unresolved));
        assert_eq!(segment.title, None);
        assert_eq!(segment.url, None);
        assert_eq!(segment.latitude, None);
        assert_eq!(segment.longitude, None);
        assert_eq!(segment.filename.as_deref(), Some("scan.jpg"));
        assert_eq!(segment.path.as_deref(), Some("/tmp/scan.jpg"));
    }

    #[test]
    fn attachment_segment_serializes_title_and_url_for_web_preview() {
        let segment = AttachmentSegment {
            index: 1,
            kind: AttachmentKind::WebPreview,
            title: Some("Cursor Docs".into()),
            url: Some("https://docs.cursor.com/guides/working-with-context".into()),
            latitude: None,
            longitude: None,
            filename: None,
            path: None,
            sha256: None,
            ocr_text: None,
            has_ocr: false,
        };

        let value = serde_json::to_value(segment).expect("serialize web preview");

        assert_eq!(value["kind"], "web_preview");
        assert_eq!(value["title"], "Cursor Docs");
        assert_eq!(
            value["url"],
            "https://docs.cursor.com/guides/working-with-context"
        );
    }

    #[test]
    fn attachment_segment_serializes_coordinates_for_map() {
        let segment = AttachmentSegment {
            index: 1,
            kind: AttachmentKind::Map,
            title: Some("屏東縣立棒球場".into()),
            url: Some("https://maps.apple.com/place?...".into()),
            latitude: Some(22.657349),
            longitude: Some(120.485956),
            filename: None,
            path: None,
            sha256: None,
            ocr_text: None,
            has_ocr: false,
        };

        let value = serde_json::to_value(segment).expect("serialize map");

        assert_eq!(value["kind"], "map");
        assert_eq!(value["latitude"], 22.657349);
        assert_eq!(value["longitude"], 120.485956);
    }
}
