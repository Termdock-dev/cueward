use std::path::PathBuf;
use std::process::Command;

use crate::applescript::escape;
use crate::send;
use crate::MacosError;

pub struct QuickNote {
    pub title: String,
    pub folder: String,
}

fn db_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home)
        .join("Library/Group Containers/group.com.apple.notes/NoteStore.sqlite")
}

/// Run a read-only SQLite query via the sqlite3 CLI to get consistent WAL reads.
fn query_db(sql: &str) -> Result<String, MacosError> {
    let path = db_path();
    let output = Command::new("/usr/bin/sqlite3")
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
           AND n.ZMARKEDFORDELETION != 1
           AND f.ZTITLE2 != 'Recently Deleted'",
    )?;

    let notes = raw
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let mut parts = line.splitn(2, '|');
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
    let escaped = title.replace('\'', "''");
    let raw = query_db(&format!(
        "SELECT f.ZTITLE2
         FROM ZICCLOUDSYNCINGOBJECT n
         JOIN ZICCLOUDSYNCINGOBJECT f ON n.ZFOLDER = f.Z_PK
         WHERE n.Z_ENT = 11
           AND n.ZISSYSTEMPAPER = 1
           AND n.ZMARKEDFORDELETION != 1
           AND f.ZTITLE2 != 'Recently Deleted'
           AND n.ZTITLE1 = '{escaped}'
         LIMIT 1"
    ))?;

    raw.lines()
        .next()
        .map(|s| s.to_string())
        .ok_or_else(|| MacosError::Other(format!("quick note not found: {title}")))
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
    let escaped_body = escape(body);

    let script = format!(
        r#"
        tell application "Notes"
            set theNote to (first note of folder "{escaped_folder}" whose name is "{escaped_title}")
            set body of theNote to "<h1>{escaped_title}</h1><br>{escaped_body}"
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
            "failed to update quick note: {stderr}"
        )));
    }

    Ok(())
}

/// Delete a Quick Note.
pub fn delete(title: &str) -> Result<(), MacosError> {
    let folder = find_folder(title)?;
    send::delete_note(title, &folder)
}
