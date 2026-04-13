use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags, params};

use crate::MacosError;
use crate::applescript::{escape, run};
use crate::notes::crud;

#[derive(serde::Serialize)]
pub struct QuickNote {
    pub title: String,
    pub folder: String,
}

fn list_by_title(title: &str) -> Result<Vec<QuickNote>, MacosError> {
    query_notes(Some(title))
}

fn db_path() -> Result<PathBuf, MacosError> {
    let home = std::env::var("HOME")
        .map_err(|_| MacosError::Other("HOME environment variable not set".to_string()))?;
    Ok(PathBuf::from(home).join("Library/Group Containers/group.com.apple.notes/NoteStore.sqlite"))
}

fn open_db() -> Result<Connection, MacosError> {
    let path = db_path()?;
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|err| MacosError::Other(format!("failed to open NoteStore.sqlite: {err}")))
}

fn query_notes(title_filter: Option<&str>) -> Result<Vec<QuickNote>, MacosError> {
    let conn = open_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT n.ZTITLE1, f.ZTITLE2
         FROM ZICCLOUDSYNCINGOBJECT n
         LEFT JOIN ZICCLOUDSYNCINGOBJECT f ON n.ZFOLDER = f.Z_PK
         WHERE n.Z_ENT = 11
           AND n.ZISSYSTEMPAPER = 1
           AND COALESCE(n.ZMARKEDFORDELETION, 0) != 1
           AND (f.ZTITLE2 IS NULL OR f.ZTITLE2 != 'Recently Deleted')
           AND (?1 IS NULL OR n.ZTITLE1 = ?1)",
        )
        .map_err(|err| MacosError::Other(format!("failed to prepare quick note query: {err}")))?;

    let rows = stmt
        .query_map(params![title_filter], |row| {
            let title: String = row.get(0)?;
            let folder: Option<String> = row.get(1)?;
            Ok((title, folder.unwrap_or_default()))
        })
        .map_err(|err| MacosError::Other(format!("failed to query quick notes: {err}")))?;

    let mut notes = Vec::new();
    for row in rows {
        let (title, folder) =
            row.map_err(|err| MacosError::Other(format!("failed to read quick note row: {err}")))?;
        if title.is_empty() {
            continue;
        }
        notes.push(QuickNote { title, folder });
    }

    Ok(notes)
}

/// List all Quick Notes (notes with ZISSYSTEMPAPER = 1).
pub fn list() -> Result<Vec<QuickNote>, MacosError> {
    query_notes(None)
}

/// Find a Quick Note's folder by title.
fn find_folder(title: &str) -> Result<String, MacosError> {
    list_by_title(title)?
        .into_iter()
        .next()
        .map(|note| note.folder)
        .ok_or_else(|| MacosError::Other(format!("quick note not found: {title}")))
}

fn find_unique(title: &str) -> Result<QuickNote, MacosError> {
    let matches = list_by_title(title)?;
    match matches.len() {
        0 => Err(MacosError::Other(format!("quick note not found: {title}"))),
        1 => Ok(matches.into_iter().next().unwrap()),
        count => Err(MacosError::Other(format!(
            "quick note title is ambiguous: {title} ({count} matches)"
        ))),
    }
}

/// Create a note in the Quick Notes folder.
///
/// Note: this creates a regular note in the "Quick Notes" folder.
/// It will NOT appear in the system Quick Notes smart folder (快速備忘錄)
/// — that requires creating via the macOS Quick Note gesture.
pub fn create(title: &str, body: &str) -> Result<(), MacosError> {
    crud::create_note(title, body, "Quick Notes")
}

/// Update a Quick Note's body (preserves title).
pub fn update(title: &str, body: &str) -> Result<(), MacosError> {
    let folder = find_folder(title)?;
    let escaped_title = escape(title);
    let escaped_folder = escape(&folder);
    let body_expr = format!("\"{}\"", escape(&html_body_for_update(title, body)));

    let script = format!(
        r#"
        tell application "Notes"
            set theNote to (first note of folder "{escaped_folder}" whose name is "{escaped_title}")
            set body of theNote to {body_expr}
            set name of theNote to "{escaped_title}"
        end tell
        "#
    );

    run(&script, "failed to update quick note")
}

/// Delete a Quick Note.
pub fn delete(title: &str) -> Result<(), MacosError> {
    let folder = find_folder(title)?;
    crud::delete_note(title, &folder)
}

fn read_body_html(title: &str, folder: &str) -> Result<String, MacosError> {
    let escaped_title = escape(title);
    let escaped_folder = escape(folder);

    let script = format!(
        r#"
        tell application "Notes"
            set theNote to (first note of folder "{escaped_folder}" whose name is "{escaped_title}")
            return body of theNote
        end tell
        "#
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| MacosError::Other(format!("osascript: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MacosError::Other(format!(
            "failed to read quick note body: {stderr}"
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_string())
}

fn escape_html_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn html_body_for_update(title: &str, body: &str) -> String {
    let html_title = escape_html_text(title);
    let body_html = if body.is_empty() {
        String::new()
    } else {
        body.split('\n')
            .map(|line| {
                if line.is_empty() {
                    "<div><br></div>".to_string()
                } else {
                    format!("<div>{}</div>", escape_html_text(line))
                }
            })
            .collect::<Vec<_>>()
            .join("")
    };

    format!("<h1>{html_title}</h1>{body_html}")
}

fn strip_title_block(title: &str, body_html: &str) -> String {
    let title_div = format!("<div>{}</div>", escape_html_text(title));
    if let Some(rest) = body_html.strip_prefix(&title_div) {
        return rest.to_string();
    }
    body_html.to_string()
}

fn ensure_archive_destination(source_folder: &str, to_folder: &str) -> Result<(), MacosError> {
    if source_folder == to_folder {
        return Err(MacosError::Other(format!(
            "archive destination must differ from the current quick note folder: {to_folder}"
        )));
    }
    Ok(())
}

pub fn archive(title: &str, to_folder: &str) -> Result<(), MacosError> {
    let note = find_unique(title)?;
    ensure_archive_destination(&note.folder, to_folder)?;
    let body_html = read_body_html(&note.title, &note.folder)?;
    let body = strip_title_block(&note.title, &body_html);

    crud::create_note(&note.title, &body, to_folder)?;
    crud::delete_note(&note.title, &note.folder)?;

    // Notes/CloudKit updates can lag a bit after delete; poll longer before
    // concluding that the quick note is still present.
    for _ in 0..20 {
        if list_by_title(title)?.is_empty() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(500));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ensure_archive_destination, html_body_for_update, strip_title_block};

    #[test]
    fn strip_title_block_removes_leading_title_div() {
        let html = "<div>Step 1</div><div>body line 1</div><div>body line 2</div>";
        let body = strip_title_block("Step 1", html);
        assert_eq!(body, "<div>body line 1</div><div>body line 2</div>");
    }

    #[test]
    fn strip_title_block_preserves_links_and_blank_lines() {
        let html = "<div>456</div>\n<div><br></div>\n<div><a href=https://example.com>https://example.com</a><br></div>";
        let body = strip_title_block("456", html);
        assert_eq!(
            body,
            "\n<div><br></div>\n<div><a href=https://example.com>https://example.com</a><br></div>"
        );
    }

    #[test]
    fn archive_same_folder_returns_clear_error() {
        let err = ensure_archive_destination("Notes", "Notes").unwrap_err();
        assert_eq!(
            err.to_string(),
            "archive destination must differ from the current quick note folder: Notes"
        );
    }

    #[test]
    fn html_body_for_update_escapes_title_and_body_lines() {
        let html = html_body_for_update("A < B", "x < y & z\n\nq > r");

        assert_eq!(
            html,
            "<h1>A &lt; B</h1><div>x &lt; y &amp; z</div><div><br></div><div>q &gt; r</div>"
        );
    }
}
