use plist::{Dictionary, Value};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::MacosError;

const SAVED_STATE_FILENAME: &str = ".SavedStickiesState";
const TITLE_KEY: &str = "CuewardTitle";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct StickiesNote {
    pub id: String,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone)]
struct StickyStateEntry {
    id: String,
    raw: Dictionary,
}

fn stickies_root_dir() -> Result<PathBuf, MacosError> {
    let home = std::env::var("HOME")
        .map_err(|_| MacosError::Other("HOME environment variable must be set".into()))?;
    Ok(PathBuf::from(home).join("Library/Containers/com.apple.Stickies/Data/Library/Stickies"))
}

fn saved_state_path(root: &Path) -> PathBuf {
    root.join(SAVED_STATE_FILENAME)
}

fn sticky_dir(root: &Path, id: &str) -> PathBuf {
    root.join(format!("{id}.rtfd"))
}

fn sticky_rtf_path(root: &Path, id: &str) -> PathBuf {
    sticky_dir(root, id).join("TXT.rtf")
}

fn parse_saved_state_value(value: Value) -> Result<Vec<StickyStateEntry>, MacosError> {
    let entries = value
        .into_array()
        .ok_or_else(|| MacosError::Other("stickies state is not an array".into()))?;

    let mut parsed = Vec::new();
    for entry in entries {
        let raw = entry
            .into_dictionary()
            .ok_or_else(|| MacosError::Other("stickies state entry is not a dictionary".into()))?;
        let id = raw
            .get("UUID")
            .and_then(Value::as_string)
            .map(str::to_string)
            .ok_or_else(|| MacosError::Other("stickies state entry missing UUID".into()))?;
        parsed.push(StickyStateEntry { id, raw });
    }

    Ok(parsed)
}

fn parse_saved_state(path: &Path) -> Result<Vec<StickyStateEntry>, MacosError> {
    let value = Value::from_file(path)
        .map_err(|err| MacosError::Other(format!("failed to decode stickies state: {err}")))?;
    parse_saved_state_value(value)
}

fn write_saved_state(path: &Path, entries: &[StickyStateEntry]) -> Result<(), MacosError> {
    let value = Value::Array(
        entries
            .iter()
            .map(|entry| Value::Dictionary(entry.raw.clone()))
            .collect(),
    );
    value
        .to_file_xml(path)
        .map_err(|err| MacosError::Other(format!("failed to write stickies state: {err}")))
}

fn ensure_update_fields(title: Option<&str>, body: Option<&str>) -> Result<(), MacosError> {
    if title.is_none() && body.is_none() {
        return Err(MacosError::Other("no sticky updates specified".into()));
    }
    Ok(())
}

fn derive_sticky_title(stored_title: Option<&str>, body: &str, id: &str) -> String {
    stored_title
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            body.lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| format!("Sticky {}", &id[..id.len().min(8)]))
}

fn read_sticky_body(path: &Path) -> Result<String, MacosError> {
    let output = Command::new("textutil")
        .arg("-convert")
        .arg("txt")
        .arg("-stdout")
        .arg(path)
        .output()
        .map_err(|err| MacosError::Other(format!("textutil: {err}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MacosError::Other(format!("textutil failed: {stderr}")));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn write_sticky_body(path: &Path, body: &str) -> Result<(), MacosError> {
    let parent = path
        .parent()
        .ok_or_else(|| MacosError::Other("invalid sticky path".into()))?;
    fs::create_dir_all(parent)
        .map_err(|err| MacosError::Other(format!("failed to create sticky dir: {err}")))?;

    let mut child = Command::new("textutil")
        .arg("-convert")
        .arg("rtf")
        .arg("-stdin")
        .arg("-stdout")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| MacosError::Other(format!("textutil: {err}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin
            .write_all(body.as_bytes())
            .map_err(|err| MacosError::Other(format!("failed to write sticky body: {err}")))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|err| MacosError::Other(format!("textutil: {err}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MacosError::Other(format!("textutil failed: {stderr}")));
    }

    fs::write(path, output.stdout)
        .map_err(|err| MacosError::Other(format!("failed to write sticky rtf: {err}")))
}

fn list_stickies_from_root(root: &Path) -> Result<Vec<StickiesNote>, MacosError> {
    let state_path = saved_state_path(root);
    if !state_path.exists() {
        return Err(MacosError::Other("stickies state not found".into()));
    }

    let entries = parse_saved_state(&state_path)?;
    let mut notes = Vec::new();
    for entry in entries {
        let body_path = sticky_rtf_path(root, &entry.id);
        if !body_path.exists() {
            continue;
        }
        let body = read_sticky_body(&body_path)?;
        let stored_title = entry.raw.get(TITLE_KEY).and_then(Value::as_string);
        let title = derive_sticky_title(stored_title, &body, &entry.id);
        notes.push(StickiesNote {
            id: entry.id,
            title,
            body,
        });
    }

    Ok(notes)
}

fn default_state_entry(id: &str, title: &str) -> StickyStateEntry {
    let mut raw = Dictionary::new();
    raw.insert("UUID".into(), Value::String(id.into()));
    raw.insert("ExpandedSize".into(), Value::String("{300, 200}".into()));
    raw.insert("ExpandFrameY".into(), Value::Integer(0.into()));
    raw.insert("Floating".into(), Value::Integer(0.into()));
    raw.insert("Frame".into(), Value::String("{{200, 900}, {300, 200}}".into()));
    raw.insert("SpellCheckingTypes".into(), Value::Integer(9191.into()));
    raw.insert("Translucent".into(), Value::Integer(0.into()));
    raw.insert("ZOrder".into(), Value::Integer(1.into()));
    raw.insert(TITLE_KEY.into(), Value::String(title.into()));
    StickyStateEntry { id: id.into(), raw }
}

fn clone_state_entry(
    template: Option<&StickyStateEntry>,
    id: &str,
    title: &str,
    z_order: i64,
) -> StickyStateEntry {
    match template {
        Some(template) => {
            let mut raw = template.raw.clone();
            raw.insert("UUID".into(), Value::String(id.into()));
            raw.insert(TITLE_KEY.into(), Value::String(title.into()));
            raw.insert("ZOrder".into(), Value::Integer(z_order.into()));
            if let Some(frame) = raw.get("Frame").and_then(Value::as_string) {
                raw.insert("Frame".into(), Value::String(offset_frame(frame)));
            }
            StickyStateEntry { id: id.into(), raw }
        }
        None => default_state_entry(id, title),
    }
}

fn max_z_order(entries: &[StickyStateEntry]) -> i64 {
    entries
        .iter()
        .filter_map(|entry| entry.raw.get("ZOrder"))
        .filter_map(|value| value.as_signed_integer())
        .max()
        .unwrap_or(0)
}

fn offset_frame(frame: &str) -> String {
    let Some((position, size)) = frame
        .strip_prefix("{{")
        .and_then(|value| value.strip_suffix("}}"))
        .and_then(|value| value.split_once("}, {"))
    else {
        return frame.to_string();
    };
    let Some((x, y)) = position.split_once(", ") else {
        return frame.to_string();
    };
    let Some((width, height)) = size.split_once(", ") else {
        return frame.to_string();
    };
    let Ok(x) = x.parse::<i32>() else {
        return frame.to_string();
    };
    let Ok(y) = y.parse::<i32>() else {
        return frame.to_string();
    };

    format!("{{{{{}, {}}}, {{{}, {}}}}}", x + 24, y - 24, width, height)
}

fn load_state_entries_for_mutation(root: &Path) -> Result<Vec<StickyStateEntry>, MacosError> {
    let state_path = saved_state_path(root);
    if !state_path.exists() {
        return Ok(Vec::new());
    }
    parse_saved_state(&state_path)
}

fn find_entry_index(entries: &[StickyStateEntry], id: &str) -> Result<usize, MacosError> {
    entries
        .iter()
        .position(|entry| entry.id == id)
        .ok_or_else(|| MacosError::Other(format!("sticky not found: {id}")))
}

fn create_sticky_in_root(root: &Path, title: &str, body: &str) -> Result<StickiesNote, MacosError> {
    let id = uuid::Uuid::new_v4().to_string().to_uppercase();
    let mut entries = load_state_entries_for_mutation(root)?;
    let next_z_order = max_z_order(&entries) + 1;
    let template = entries.first().cloned();
    entries.push(clone_state_entry(template.as_ref(), &id, title, next_z_order));
    let body_path = sticky_rtf_path(root, &id);
    write_sticky_body(&body_path, body)?;
    if let Err(err) = write_saved_state(&saved_state_path(root), &entries) {
        let _ = fs::remove_dir_all(sticky_dir(root, &id));
        return Err(err);
    }

    Ok(StickiesNote {
        id,
        title: title.to_string(),
        body: body.to_string(),
    })
}

fn update_sticky_in_root(
    root: &Path,
    id: &str,
    title: Option<&str>,
    body: Option<&str>,
) -> Result<StickiesNote, MacosError> {
    ensure_update_fields(title, body)?;

    let mut entries = load_state_entries_for_mutation(root)?;
    let index = find_entry_index(&entries, id)?;
    let body_path = sticky_rtf_path(root, id);
    if !body_path.exists() {
        return Err(MacosError::Other(format!("sticky body not found: {id}")));
    }

    let current_body = read_sticky_body(&body_path)?;
    let original_raw = entries[index].raw.clone();
    let next_body = body.unwrap_or(&current_body).to_string();

    if let Some(title) = title {
        entries[index]
            .raw
            .insert(TITLE_KEY.into(), Value::String(title.to_string()));
    }

    if body.is_some() {
        write_sticky_body(&body_path, &next_body)?;
    }
    if let Err(err) = write_saved_state(&saved_state_path(root), &entries) {
        entries[index].raw = original_raw;
        if body.is_some() {
            let _ = write_sticky_body(&body_path, &current_body);
        }
        return Err(err);
    }

    let next_title = entries[index]
        .raw
        .get(TITLE_KEY)
        .and_then(Value::as_string)
        .map(str::to_string)
        .unwrap_or_else(|| derive_sticky_title(None, &next_body, id));

    Ok(StickiesNote {
        id: id.to_string(),
        title: next_title,
        body: next_body,
    })
}

fn delete_sticky_in_root(root: &Path, id: &str) -> Result<(), MacosError> {
    let mut entries = load_state_entries_for_mutation(root)?;
    let original_len = entries.len();
    entries.retain(|entry| entry.id != id);
    if entries.len() == original_len {
        return Err(MacosError::Other(format!("sticky not found: {id}")));
    }

    write_saved_state(&saved_state_path(root), &entries)?;
    let dir = sticky_dir(root, id);
    if dir.exists() {
        fs::remove_dir_all(&dir)
            .map_err(|err| MacosError::Other(format!("failed to delete sticky dir: {err}")))?;
    }
    Ok(())
}

/// List all Stickies notes.
pub fn list_stickies() -> Result<Vec<StickiesNote>, MacosError> {
    list_stickies_from_root(&stickies_root_dir()?)
}

/// Create a Stickies note.
pub fn create_sticky(title: &str, body: &str) -> Result<StickiesNote, MacosError> {
    create_sticky_in_root(&stickies_root_dir()?, title, body)
}

/// Update a Stickies note by UUID.
pub fn update_sticky(id: &str, title: Option<&str>, body: Option<&str>) -> Result<StickiesNote, MacosError> {
    update_sticky_in_root(&stickies_root_dir()?, id, title, body)
}

/// Delete a Stickies note by UUID.
pub fn delete_sticky(id: &str) -> Result<(), MacosError> {
    delete_sticky_in_root(&stickies_root_dir()?, id)
}

#[cfg(test)]
mod tests;
