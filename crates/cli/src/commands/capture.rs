use std::process;

use chrono::Utc;
use cueward_adapter_macos::MacosAdapter;
use cueward_core::{PlatformAdapter, State, inbox, storage};

use super::Source;
use super::helpers::{parse_duration, print_external, source_name};

pub(crate) fn dispatch(source: Source, since: String, incremental: bool) {
    if let Err(error) = run(source, &since, incremental) {
        eprintln!("error: {error}");
        process::exit(1);
    }
}

fn run(source: Source, since: &str, incremental: bool) -> Result<(), String> {
    let duration = parse_duration(since)
        .filter(|d| *d > chrono::Duration::zero())
        .ok_or_else(|| {
            format!("invalid duration '{since}' (use a positive duration such as 24h, 7d, 30m)")
        })?;
    let since_dt = Utc::now()
        .checked_sub_signed(duration)
        .ok_or("duration is out of range")?;
    let _lock = storage::lock("capture").map_err(|e| e.to_string())?;
    let mut state = State::try_load().map_err(|e| format!("failed to load state: {e}"))?;
    let sources = match source {
        Source::All => vec![Source::Safari, Source::Notes, Source::Messages],
        other => vec![other],
    };
    let (cues, mut errors) = capture_sources(sources, &mut state, since_dt, incremental);
    // Commit the data before its checkpoint. A retry after state failure is deduplicated by triage.
    if !cues.is_empty() || errors.is_empty() {
        match inbox::save(&cues) {
            Ok(path) => {
                eprintln!("saved to {}", path.display());
                if let Err(e) = state.save() {
                    errors.push(format!("failed to save state: {e}"));
                }
            }
            Err(e) => errors.push(format!("failed to save inbox: {e}")),
        }
    }
    print_external(
        "capture",
        &serde_json::to_string_pretty(&cues).map_err(|e| e.to_string())?,
    );
    if errors.is_empty() {
        eprintln!("captured {} cues", cues.len());
        Ok(())
    } else {
        Err(format!(
            "capture incomplete ({} cues read): {}",
            cues.len(),
            errors.join("; ")
        ))
    }
}

fn capture_sources(
    sources: Vec<Source>,
    state: &mut State,
    since_dt: chrono::DateTime<Utc>,
    incremental: bool,
) -> (Vec<cueward_core::Cue>, Vec<String>) {
    let mut cues = Vec::new();
    let mut errors = Vec::new();
    let adapter = MacosAdapter;
    for src in sources {
        let name = source_name(&src);
        let cutoff = source_cutoff(state, name, since_dt, incremental);
        let result = match src {
            Source::Safari => adapter.capture_browser_history(cutoff),
            Source::Notes => adapter.capture_notes(cutoff),
            Source::Messages => adapter.capture_messages(cutoff),
            Source::All => unreachable!(),
        };
        match result {
            Ok(batch) => {
                if let Some(timestamp) = batch.iter().map(|c| c.timestamp).max() {
                    state.set_watermark(name, timestamp);
                }
                cues.extend(batch);
            }
            Err(e) => errors.push(format!("{name}: {e}")),
        }
    }
    (cues, errors)
}

fn source_cutoff(
    state: &State,
    source: &str,
    bootstrap: chrono::DateTime<Utc>,
    incremental: bool,
) -> chrono::DateTime<Utc> {
    if incremental {
        state
            .watermark(source)
            .and_then(|time| time.checked_sub_signed(chrono::Duration::seconds(1)))
            .unwrap_or(bootstrap)
    } else {
        bootstrap
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    #[test]
    fn incremental_cutoff_replays_boundary_and_bootstraps_each_source_independently() {
        let now = Utc::now();
        let bootstrap = now - chrono::Duration::days(1);
        let watermark = now - chrono::Duration::days(3);
        let mut state = State::default();
        state.set_watermark("notes", watermark);
        assert_eq!(
            source_cutoff(&state, "notes", bootstrap, true),
            watermark - chrono::Duration::seconds(1)
        );
        assert_eq!(source_cutoff(&state, "safari", bootstrap, true), bootstrap);
        assert_eq!(source_cutoff(&state, "notes", bootstrap, false), bootstrap);
        assert!(
            super::super::Cli::try_parse_from([
                "cueward",
                "capture",
                "--incremental",
                "--since",
                "7d"
            ])
            .is_ok()
        );
    }
}
