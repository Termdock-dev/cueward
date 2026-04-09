use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

use crate::applescript::{escape, escape_body, run};
use crate::send;
use crate::MacosError;

#[derive(serde::Serialize)]
pub struct QuickNote {
    pub title: String,
    pub folder: String,
}

fn list_by_title(title: &str) -> Result<Vec<QuickNote>, MacosError> {
    let escaped = title.replace('\'', "''");
    let raw = query_db(&format!(
        "SELECT n.ZTITLE1, f.ZTITLE2
         FROM ZICCLOUDSYNCINGOBJECT n
         LEFT JOIN ZICCLOUDSYNCINGOBJECT f ON n.ZFOLDER = f.Z_PK
         WHERE n.Z_ENT = 11
           AND n.ZISSYSTEMPAPER = 1
           AND COALESCE(n.ZMARKEDFORDELETION, 0) != 1
           AND (f.ZTITLE2 IS NULL OR f.ZTITLE2 != 'Recently Deleted')
           AND n.ZTITLE1 = '{escaped}'"
    ))?;

    Ok(raw
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let mut parts = line.splitn(2, '\t');
            let title = parts.next()?.to_string();
            let folder = parts.next().unwrap_or("").to_string();
            if title.is_empty() {
                return None;
            }
            Some(QuickNote { title, folder })
        })
        .collect())
}

fn db_path() -> Result<PathBuf, MacosError> {
    let home = std::env::var("HOME")
        .map_err(|_| MacosError::Other("HOME environment variable not set".to_string()))?;
    Ok(PathBuf::from(home)
        .join("Library/Group Containers/group.com.apple.notes/NoteStore.sqlite"))
}

/// Run a read-only SQLite query via the sqlite3 CLI to get consistent WAL reads.
fn query_db(sql: &str) -> Result<String, MacosError> {
    let path = db_path()?;
    let output = Command::new("/usr/bin/sqlite3")
        .arg("-readonly")
        .arg("-separator")
        .arg("\t")
        .arg(path)
        .arg(sql)
        .output()
        .map_err(|e| MacosError::Other(format!("sqlite3: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MacosError::Other(format!("sqlite3 error: {stderr}")));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// List all Quick Notes (notes with ZISSYSTEMPAPER = 1).
pub fn list() -> Result<Vec<QuickNote>, MacosError> {
    let raw = query_db(
        "SELECT n.ZTITLE1, f.ZTITLE2
         FROM ZICCLOUDSYNCINGOBJECT n
         LEFT JOIN ZICCLOUDSYNCINGOBJECT f ON n.ZFOLDER = f.Z_PK
         WHERE n.Z_ENT = 11
           AND n.ZISSYSTEMPAPER = 1
           AND COALESCE(n.ZMARKEDFORDELETION, 0) != 1
           AND (f.ZTITLE2 IS NULL OR f.ZTITLE2 != 'Recently Deleted')",
    )?;

    let notes = raw
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let mut parts = line.splitn(2, '\t');
            let title = parts.next()?.to_string();
            let folder = parts.next().unwrap_or("").to_string();
            if title.is_empty() {
                return None;
            }
            Some(QuickNote { title, folder })
        })
        .collect();

    Ok(notes)
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
    send::create_note(title, body, "Quick Notes")
}

/// Update a Quick Note's body (preserves title).
pub fn update(title: &str, body: &str) -> Result<(), MacosError> {
    let folder = find_folder(title)?;
    let escaped_title = escape(title);
    let escaped_folder = escape(&folder);
    let html_title = title.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let body_expr = escape_body(&format!("<h1>{html_title}</h1><br>{body}"));

    let script = format!(
        r#"
        tell application "Notes"
            set theNote to (first note of folder "{escaped_folder}" whose name is "{escaped_title}")
            set body of theNote to {body_expr}
        end tell
        "#
    );

    run(&script, "failed to update quick note")
}

/// Delete a Quick Note.
pub fn delete(title: &str) -> Result<(), MacosError> {
    let folder = find_folder(title)?;
    send::delete_note(title, &folder)
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

    Ok(String::from_utf8_lossy(&output.stdout).trim_end().to_string())
}

fn escape_html_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn strip_title_block(title: &str, body_html: &str) -> String {
    let title_div = format!("<div>{}</div>", escape_html_text(title));
    if let Some(rest) = body_html.strip_prefix(&title_div) {
        return rest.to_string();
    }
    body_html.to_string()
}

pub fn archive(title: &str, to_folder: &str) -> Result<(), MacosError> {
    let note = find_unique(title)?;
    let body_html = read_body_html(&note.title, &note.folder)?;
    let body = strip_title_block(&note.title, &body_html);

    send::create_note(&note.title, &body, to_folder)?;
    send::delete_note(&note.title, &note.folder)?;

    // Notes/CloudKit updates can lag a bit after delete; poll longer before
    // concluding that the quick note is still present.
    for _ in 0..20 {
        if list_by_title(title)?.is_empty() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(500));
    }

    Err(MacosError::Other(format!(
        "quick note still present after archive: {title}"
    )))
}

#[cfg(test)]
mod tests {
    use super::strip_title_block;

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
}
