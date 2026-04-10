use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::{DateTime, TimeZone, Utc};
use rusqlite::Connection;
use sha2::{Digest, Sha256};

use cueward_core::{AttachmentSegment, Cue, CueSource};

use crate::MacosError;
use crate::ocr;

const ATTACHMENT_PLACEHOLDER: char = '\u{fffc}';
const ATTACHMENT_LABEL: &str = "[Attachment]";
const MEDIA_MATCH_WINDOW_SECS: i64 = 5;
const APPLE_EPOCH_OFFSET: f64 = 978_307_200.0;

pub fn capture(since: DateTime<Utc>) -> Result<Vec<Cue>, MacosError> {
    let seconds_ago = (Utc::now() - since).num_seconds().max(0);

    // Compute unix timestamps in AppleScript by getting the current unix time via
    // `date +%s` and subtracting the delta between `current date` and `modification date`.
    // This avoids locale-dependent date formatting and timezone offset issues.
    let script = format!(
        r#"
        set output to ""
        set sinceDate to (current date) - {seconds_ago}
        set nowDate to current date
        tell application "Notes"
            set allNotes to every note
            repeat with theNote in allNotes
                try
                    set modDate to modification date of theNote
                    if modDate > sinceDate then
                        set noteName to name of theNote
                        set noteBody to plaintext of theNote
                        try
                            set theContainer to container of theNote
                            set noteFolder to name of theContainer
                        on error
                            set noteFolder to "Unknown"
                        end try
                        set secsDelta to nowDate - modDate
                        set unixStr to do shell script "echo $(( $(date +%s) - " & secsDelta & " ))"
                        set output to output & "---CUE_SEP---" & unixStr & "---FIELD---" & noteName & "---FIELD---" & noteFolder & "---FIELD---" & noteBody
                    end if
                end try
            end repeat
        end tell
        return output
        "#
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| MacosError::PermissionDenied(format!("osascript: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not allowed") || stderr.contains("denied") {
            return Err(MacosError::PermissionDenied(
                "Apple Notes access denied. Allow automation in System Settings > Privacy & Security > Automation".into(),
            ));
        }
        return Err(MacosError::PermissionDenied(format!("osascript error: {stderr}")));
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    let mut cues: Vec<Cue> = raw
        .split("---CUE_SEP---")
        .filter(|s| !s.trim().is_empty())
        .filter_map(|entry| {
            let fields: Vec<&str> = entry.splitn(4, "---FIELD---").collect();
            if fields.len() < 4 {
                return None;
            }
            let unix_ts: i64 = fields[0].trim().parse().ok()?;
            let timestamp = Utc.timestamp_opt(unix_ts, 0).single()?;
            let title = fields[1].trim().to_string();
            let folder = fields[2].trim().to_string();
            let (body, _) = normalize_plaintext(fields[3].trim());

            let metadata = HashMap::from([("folder".into(), folder)]);

            Some(Cue {
                source: CueSource::Notes,
                timestamp,
                content: body,
                url: None,
                title: Some(title),
                tags: Vec::new(),
                attachment_segments: Vec::new(),
                metadata,
            })
        })
        .collect();

    if let Ok(media_notes) = load_media_notes() {
        enrich_cues_with_media(&mut cues, &media_notes);
        enrich_cues_with_attachment_ocr(&mut cues, &media_notes);
    }

    Ok(cues)
}

fn normalize_plaintext(body: &str) -> (String, usize) {
    let attachment_placeholders = body.chars().filter(|c| *c == ATTACHMENT_PLACEHOLDER).count();
    if attachment_placeholders == 0 {
        return (body.to_string(), 0);
    }

    (
        body.replace(ATTACHMENT_PLACEHOLDER, ATTACHMENT_LABEL),
        attachment_placeholders,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MediaAttachment {
    filename: String,
    path: PathBuf,
    sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MediaNote {
    timestamp: i64,
    title: Option<String>,
    attachments: Vec<MediaAttachment>,
}

fn enrich_cues_with_media(cues: &mut [Cue], media_notes: &[MediaNote]) {
    for cue in cues.iter_mut() {
        if !matches!(cue.source, CueSource::Notes) {
            continue;
        }

        let placeholder_count = attachment_placeholder_count(&cue.content);
        if placeholder_count == 0 {
            continue;
        }

        let Some(media_note) = match_media_note(cue, media_notes) else {
            continue;
        };

        if media_note.attachments.is_empty() {
            continue;
        }

        cue.content = replace_attachment_labels(
            &cue.content,
            &media_note.attachments,
            placeholder_count,
        );

        cue.attachment_segments =
            build_attachment_segments(&media_note.attachments, placeholder_count, None);
    }
}

fn match_media_note<'a>(cue: &Cue, media_notes: &'a [MediaNote]) -> Option<&'a MediaNote> {
    let cue_ts = cue.timestamp.timestamp();
    let cue_title = cue.title.as_deref();

    media_notes
        .iter()
        .filter(|note| (note.timestamp - cue_ts).abs() <= MEDIA_MATCH_WINDOW_SECS)
        .min_by_key(|note| {
            let title_penalty = match (cue_title, note.title.as_deref()) {
                (Some(cue_title), Some(note_title)) if cue_title == note_title => 0,
                (Some(_), Some(_)) => 1,
                _ => 2,
            };
            (title_penalty, (note.timestamp - cue_ts).abs())
        })
}

fn replace_attachment_labels(
    body: &str,
    attachments: &[MediaAttachment],
    placeholder_count: usize,
) -> String {
    if placeholder_count == 0 {
        return body.to_string();
    }

    let mut result = body.to_string();
    for (idx, attachment) in attachments.iter().enumerate() {
        if idx >= placeholder_count {
            break;
        }

        let replacement = format!("[Attachment {}: {}]", idx + 1, attachment.filename);
        result = result.replacen(ATTACHMENT_LABEL, &replacement, 1);
    }

    result
}

fn enrich_cues_with_attachment_ocr(cues: &mut [Cue], media_notes: &[MediaNote]) {
    for cue in cues.iter_mut() {
        if !matches!(cue.source, CueSource::Notes) {
            continue;
        }

        let placeholder_count = cue.attachment_segments.len();
        if placeholder_count == 0 {
            continue;
        }

        let Some(media_note) = match_media_note(cue, media_notes) else {
            continue;
        };

        let ocr_blocks = collect_attachment_ocr_blocks(&media_note.attachments, placeholder_count);
        if ocr_blocks.is_empty() {
            continue;
        }

        cue.content = append_attachment_ocr(&cue.content, &ocr_blocks);
        cue.attachment_segments = build_attachment_segments(
            &media_note.attachments,
            placeholder_count,
            Some(&ocr_blocks),
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AttachmentOcrBlock {
    filename: String,
    sha256: Option<String>,
    text: String,
}

fn collect_attachment_ocr_blocks(
    attachments: &[MediaAttachment],
    placeholder_count: usize,
) -> Vec<AttachmentOcrBlock> {
    attachments
        .iter()
        .take(placeholder_count)
        .filter_map(|attachment| {
            let text = load_or_run_attachment_ocr(attachment).ok().flatten()?;
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return None;
            }

            Some(AttachmentOcrBlock {
                filename: attachment.filename.clone(),
                sha256: attachment.sha256.clone(),
                text: trimmed.to_string(),
            })
        })
        .collect()
}

fn append_attachment_ocr(content: &str, blocks: &[AttachmentOcrBlock]) -> String {
    if blocks.is_empty() {
        return content.to_string();
    }

    let mut sections = Vec::with_capacity(blocks.len());
    for (idx, block) in blocks.iter().enumerate() {
        sections.push(format!(
            "[Attachment {} OCR: {}]\n{}",
            idx + 1,
            block.filename,
            block.text
        ));
    }

    format!("{content}\n\n{}", sections.join("\n\n"))
}

fn attachment_placeholder_count(content: &str) -> usize {
    content.matches(ATTACHMENT_LABEL).count()
}

fn build_attachment_segments(
    attachments: &[MediaAttachment],
    placeholder_count: usize,
    ocr_blocks: Option<&[AttachmentOcrBlock]>,
) -> Vec<AttachmentSegment> {
    let ocr_by_filename: HashMap<&str, &AttachmentOcrBlock> = ocr_blocks
        .unwrap_or(&[])
        .iter()
        .map(|block| (block.filename.as_str(), block))
        .collect();

    attachments
        .iter()
        .take(placeholder_count)
        .enumerate()
        .map(|(idx, attachment)| {
            let ocr = ocr_by_filename.get(attachment.filename.as_str());
            AttachmentSegment {
                index: idx + 1,
                filename: attachment.filename.clone(),
                path: attachment.path.display().to_string(),
                sha256: attachment.sha256.clone(),
                ocr_text: ocr.map(|block| block.text.clone()),
                has_ocr: ocr.is_some(),
            }
        })
        .collect()
}

fn load_or_run_attachment_ocr(attachment: &MediaAttachment) -> Result<Option<String>, MacosError> {
    let cache_path = attachment
        .sha256
        .as_deref()
        .map(ocr_cache_file_path)
        .unwrap_or_else(|| {
            let sanitized = attachment.filename.replace('/', "_");
            ocr_cache_dir().join(format!("name-{sanitized}.txt"))
        });

    if let Ok(cached) = fs::read_to_string(&cache_path) {
        if cached.is_empty() {
            return Ok(None);
        }
        return Ok(Some(cached));
    }

    let cues = ocr::capture(&attachment.path.to_string_lossy())?;
    let text = cues
        .iter()
        .map(|cue| cue.content.trim())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    fs::create_dir_all(ocr_cache_dir())
        .map_err(|err| MacosError::Other(format!("failed to create OCR cache dir: {err}")))?;
    fs::write(&cache_path, &text)
        .map_err(|err| MacosError::Other(format!("failed to write OCR cache: {err}")))?;

    if text.is_empty() {
        Ok(None)
    } else {
        Ok(Some(text))
    }
}

fn ocr_cache_dir() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/Users/unknown".into()))
        .join(".cueward/cache/ocr")
}

fn ocr_cache_file_path(hash: &str) -> PathBuf {
    ocr_cache_dir().join(format!("{hash}.txt"))
}

fn load_media_notes() -> Result<Vec<MediaNote>, MacosError> {
    let note_store = notes_group_container_path().join("NoteStore.sqlite");
    let media_root = notes_group_container_path().join("Accounts");
    let conn = Connection::open(note_store)
        .map_err(|err| MacosError::Other(format!("failed to open NoteStore.sqlite: {err}")))?;

    let mut stmt = conn.prepare(
        r#"
        SELECT
            note.ZMODIFICATIONDATE,
            note.ZTITLE,
            media.ZFILENAME,
            media.ZIDENTIFIER
        FROM ZICCLOUDSYNCINGOBJECT AS note
        JOIN ZICCLOUDSYNCINGOBJECT AS media
            ON note.ZMEDIA = media.Z_PK
        WHERE note.ZMEDIA IS NOT NULL
          AND note.ZMEDIA != 0
          AND media.ZFILENAME IS NOT NULL
          AND media.ZIDENTIFIER IS NOT NULL
        "#,
    )
    .map_err(|err| MacosError::Other(format!("failed to prepare media query: {err}")))?;

    let mut rows = stmt
        .query([])
        .map_err(|err| MacosError::Other(format!("failed to query note media: {err}")))?;

    let mut grouped: HashMap<(i64, Option<String>), Vec<MediaAttachment>> = HashMap::new();
    while let Some(row) = rows
        .next()
        .map_err(|err| MacosError::Other(format!("failed to read media row: {err}")))?
    {
        let modification_date: f64 = row
            .get(0)
            .map_err(|err| MacosError::Other(format!("failed to decode modification date: {err}")))?;
        let title: Option<String> = row
            .get(1)
            .map_err(|err| MacosError::Other(format!("failed to decode note title: {err}")))?;
        let filename: String = row
            .get(2)
            .map_err(|err| MacosError::Other(format!("failed to decode attachment filename: {err}")))?;
        let identifier: String = row
            .get(3)
            .map_err(|err| MacosError::Other(format!("failed to decode attachment identifier: {err}")))?;

        let Some(path) = resolve_media_path(&media_root, &identifier, &filename) else {
            continue;
        };
        let sha256 = compute_sha256(&path);

        let timestamp = (modification_date + APPLE_EPOCH_OFFSET).round() as i64;
        grouped
            .entry((timestamp, normalize_media_title(title)))
            .or_default()
            .push(MediaAttachment {
                filename,
                path,
                sha256,
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

fn normalize_media_title(title: Option<String>) -> Option<String> {
    title.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn notes_group_container_path() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/Users/unknown".into()))
        .join("Library/Group Containers/group.com.apple.notes")
}

fn resolve_media_path(media_root: &Path, identifier: &str, filename: &str) -> Option<PathBuf> {
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
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.file_name().and_then(|name| name.to_str()) == Some(filename) {
            return Some(path);
        }
        if path.is_dir() {
            if let Some(found) = find_named_file(&path, filename) {
                return Some(found);
            }
        }
    }
    None
}

fn compute_sha256(path: &Path) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];

    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Some(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::collections::HashMap;
    use std::path::PathBuf;

    use chrono::{TimeZone, Utc};
    use cueward_core::{Cue, CueSource};

    use super::{
        append_attachment_ocr, attachment_placeholder_count, build_attachment_segments, compute_sha256,
        enrich_cues_with_media, find_named_file, normalize_plaintext, ocr_cache_file_path,
        replace_attachment_labels, AttachmentOcrBlock, MediaAttachment, MediaNote,
        ATTACHMENT_LABEL,
    };

    #[test]
    fn normalize_plaintext_replaces_attachment_placeholder_chars() {
        let body = format!("before{}after", '\u{fffc}');

        let (normalized, placeholders) = normalize_plaintext(&body);

        assert_eq!(normalized, format!("before{ATTACHMENT_LABEL}after"));
        assert_eq!(placeholders, 1);
    }

    #[test]
    fn normalize_plaintext_keeps_regular_text_unchanged() {
        let (normalized, placeholders) = normalize_plaintext("plain text note");

        assert_eq!(normalized, "plain text note");
        assert_eq!(placeholders, 0);
    }

    #[test]
    fn replace_attachment_labels_uses_filenames() {
        let body = "[Attachment]\n[Attachment]";
        let attachments = vec![
            MediaAttachment {
                filename: "one.png".into(),
                path: PathBuf::from("/tmp/one.png"),
                sha256: None,
            },
            MediaAttachment {
                filename: "two.jpg".into(),
                path: PathBuf::from("/tmp/two.jpg"),
                sha256: None,
            },
        ];

        let replaced = replace_attachment_labels(body, &attachments, 2);

        assert_eq!(replaced, "[Attachment 1: one.png]\n[Attachment 2: two.jpg]");
    }

    #[test]
    fn enrich_cues_with_media_adds_paths_and_filenames() {
        let timestamp = Utc.with_ymd_and_hms(2026, 4, 9, 23, 42, 54).unwrap();
        let mut cues = vec![Cue {
            source: CueSource::Notes,
            timestamp,
            content: "[Attachment]".into(),
            url: None,
            title: Some("新增備忘錄".into()),
            tags: Vec::new(),
            attachment_segments: Vec::new(),
            metadata: HashMap::new(),
        }];
        let media_notes = vec![MediaNote {
            timestamp: timestamp.timestamp() - 1,
            title: None,
            attachments: vec![MediaAttachment {
                filename: "scan.jpg".into(),
                path: PathBuf::from("/tmp/scan.jpg"),
                sha256: Some("abc123".into()),
            }],
        }];

        enrich_cues_with_media(&mut cues, &media_notes);

        assert_eq!(cues[0].content, "[Attachment 1: scan.jpg]");
        assert_eq!(cues[0].attachment_segments.len(), 1);
        assert_eq!(cues[0].attachment_segments[0].filename, "scan.jpg");
        assert_eq!(cues[0].attachment_segments[0].path, "/tmp/scan.jpg");
        assert_eq!(cues[0].attachment_segments[0].sha256.as_deref(), Some("abc123"));
        assert!(!cues[0].attachment_segments[0].has_ocr);
    }

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
    fn compute_sha256_returns_stable_digest() {
        let temp = tempfile::tempdir().expect("tempdir");
        let target = temp.path().join("image.png");
        fs::write(&target, b"hello world").expect("write media file");

        let digest = compute_sha256(&target);

        assert_eq!(
            digest.as_deref(),
            Some("b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9")
        );
    }

    #[test]
    fn append_attachment_ocr_adds_readable_sections() {
        let content = "[Attachment 1: image.png]";
        let blocks = vec![AttachmentOcrBlock {
            filename: "image.png".into(),
            sha256: Some("hash1".into()),
            text: "detected text".into(),
        }];

        let combined = append_attachment_ocr(content, &blocks);

        assert_eq!(
            combined,
            "[Attachment 1: image.png]\n\n[Attachment 1 OCR: image.png]\ndetected text"
        );
    }

    #[test]
    fn ocr_cache_file_path_uses_hash_name() {
        let path = ocr_cache_file_path("abc123");

        assert!(path.ends_with(".cueward/cache/ocr/abc123.txt"));
    }

    #[test]
    fn attachment_placeholder_count_counts_remaining_plain_labels() {
        assert_eq!(attachment_placeholder_count("[Attachment]\n[Attachment]"), 2);
        assert_eq!(attachment_placeholder_count("[Attachment 1: image.png]"), 0);
    }

    #[test]
    fn build_attachment_segments_includes_ocr_fields() {
        let attachments = vec![MediaAttachment {
            filename: "image.png".into(),
            path: PathBuf::from("/tmp/image.png"),
            sha256: Some("hash1".into()),
        }];
        let blocks = vec![AttachmentOcrBlock {
            filename: "image.png".into(),
            sha256: Some("hash1".into()),
            text: "detected text".into(),
        }];

        let segments = build_attachment_segments(&attachments, 1, Some(&blocks));

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].index, 1);
        assert_eq!(segments[0].filename, "image.png");
        assert_eq!(segments[0].path, "/tmp/image.png");
        assert_eq!(segments[0].sha256.as_deref(), Some("hash1"));
        assert_eq!(segments[0].ocr_text.as_deref(), Some("detected text"));
        assert!(segments[0].has_ocr);
    }
}
