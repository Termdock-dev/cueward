use std::fs;
use std::path::PathBuf;

use crate::Cue;

fn inbox_dir() -> std::io::Result<PathBuf> {
    Ok(crate::storage::data_dir()?.join("inbox"))
}

/// Durably publish a complete uniquely named batch without replacing another capture.
pub fn save(cues: &[Cue]) -> std::io::Result<PathBuf> {
    let dir = inbox_dir()?;
    fs::create_dir_all(&dir)?;
    let filename = format!("{}.json", crate::storage::unique_name());
    let path = dir.join(&filename);
    let json = serde_json::to_string_pretty(cues)?;
    crate::storage::write_atomic(&path, json.as_bytes(), false)?;
    Ok(path)
}

/// Read pending batches deterministically, rejecting malformed JSON instead of skipping it.
pub fn load_all() -> std::io::Result<Vec<(PathBuf, Vec<Cue>)>> {
    let dir = inbox_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut batches = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "json") {
            let content = fs::read_to_string(&path)?;
            match serde_json::from_str::<Vec<Cue>>(&content) {
                Ok(cues) => batches.push((path, cues)),
                Err(e) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("failed to parse {}: {e}", path.display()),
                    ));
                }
            }
        }
    }
    batches.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(batches)
}

/// Archive only after indexing; never overwrite a different archived batch.
pub fn mark_done(path: &PathBuf) -> std::io::Result<()> {
    let done_dir = crate::storage::data_dir()?.join("processed");
    fs::create_dir_all(&done_dir)?;
    let filename = path
        .file_name()
        .ok_or_else(|| std::io::Error::other("missing batch name"))?;
    let destination = done_dir.join(filename);
    match fs::hard_link(path, &destination) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            if fs::read(path)? != fs::read(&destination)? {
                return Err(e);
            }
        }
        Err(e) => return Err(e),
    }
    fs::File::open(&done_dir)?.sync_all()?;
    fs::remove_file(path)?;
    if let Some(parent) = path.parent() {
        fs::File::open(parent)?.sync_all()?;
    }
    Ok(())
}
