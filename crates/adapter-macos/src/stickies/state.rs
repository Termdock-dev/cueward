use plist::{Dictionary, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::MacosError;

pub const SAVED_STATE_FILENAME: &str = ".SavedStickiesState";
pub const TITLE_KEY: &str = "CuewardTitle";

#[derive(Debug, Clone)]
pub struct StickyStateEntry {
    pub id: String,
    pub raw: Dictionary,
}

pub fn stickies_root_dir() -> Result<PathBuf, MacosError> {
    let home = std::env::var("HOME")
        .map_err(|_| MacosError::Other("HOME environment variable must be set".into()))?;
    Ok(PathBuf::from(home).join("Library/Containers/com.apple.Stickies/Data/Library/Stickies"))
}

pub fn saved_state_path(root: &Path) -> PathBuf {
    root.join(SAVED_STATE_FILENAME)
}

pub fn sticky_dir(root: &Path, id: &str) -> PathBuf {
    root.join(format!("{id}.rtfd"))
}

pub fn sticky_rtf_path(root: &Path, id: &str) -> PathBuf {
    sticky_dir(root, id).join("TXT.rtf")
}

pub fn parse_saved_state_value(value: Value) -> Result<Vec<StickyStateEntry>, MacosError> {
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

pub fn parse_saved_state(path: &Path) -> Result<Vec<StickyStateEntry>, MacosError> {
    let value = Value::from_file(path)
        .map_err(|err| MacosError::Other(format!("failed to decode stickies state: {err}")))?;
    parse_saved_state_value(value)
}

pub fn write_saved_state(path: &Path, entries: &[StickyStateEntry]) -> Result<(), MacosError> {
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

pub fn ensure_update_fields(title: Option<&str>, body: Option<&str>) -> Result<(), MacosError> {
    if title.is_none() && body.is_none() {
        return Err(MacosError::Other("no sticky updates specified".into()));
    }
    Ok(())
}

pub fn derive_sticky_title(stored_title: Option<&str>, body: &str, id: &str) -> String {
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

pub fn read_sticky_body(path: &Path) -> Result<String, MacosError> {
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

pub fn write_sticky_body(path: &Path, body: &str) -> Result<(), MacosError> {
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

pub fn max_z_order(entries: &[StickyStateEntry]) -> i64 {
    entries
        .iter()
        .filter_map(|entry| entry.raw.get("ZOrder"))
        .filter_map(|value| value.as_signed_integer())
        .max()
        .unwrap_or(0)
}

pub fn load_state_entries_for_mutation(root: &Path) -> Result<Vec<StickyStateEntry>, MacosError> {
    let state_path = saved_state_path(root);
    if !state_path.exists() {
        return Ok(Vec::new());
    }
    parse_saved_state(&state_path)
}

pub fn find_entry_index(entries: &[StickyStateEntry], id: &str) -> Result<usize, MacosError> {
    entries
        .iter()
        .position(|entry| entry.id == id)
        .ok_or_else(|| MacosError::Other(format!("sticky not found: {id}")))
}
