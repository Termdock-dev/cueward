use std::fs;
use std::path::PathBuf;

use crate::Cue;

fn inbox_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".cueward/inbox")
}

pub fn save(cues: &[Cue]) -> std::io::Result<PathBuf> {
    let dir = inbox_dir();
    fs::create_dir_all(&dir)?;
    let filename = format!("{}.json", chrono::Utc::now().format("%Y%m%d-%H%M%S%.3f"));
    let path = dir.join(&filename);
    let json = serde_json::to_string_pretty(cues)?;
    fs::write(&path, json)?;
    Ok(path)
}

pub fn load_all() -> std::io::Result<Vec<(PathBuf, Vec<Cue>)>> {
    let dir = inbox_dir();
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
                Err(e) => eprintln!("warning: failed to parse {}: {e}", path.display()),
            }
        }
    }
    Ok(batches)
}

pub fn mark_done(path: &PathBuf) -> std::io::Result<()> {
    let done_dir = inbox_dir().parent().unwrap().join("processed");
    fs::create_dir_all(&done_dir)?;
    let filename = path.file_name().unwrap();
    fs::rename(path, done_dir.join(filename))
}
