use std::process;

use chrono::Utc;

use cueward_adapter_macos::MacosAdapter;
use cueward_core::{PlatformAdapter, State, inbox};

use super::Source;
use super::helpers::{parse_duration, print_external, source_name};

pub(crate) fn dispatch(source: Source, since: String) {
    let duration = match parse_duration(&since) {
        Some(d) => d,
        None => {
            eprintln!("error: invalid duration '{since}' (use e.g. 24h, 7d, 30m)");
            process::exit(1);
        }
    };

    let since_dt = Utc::now() - duration;
    let adapter = MacosAdapter;
    let mut all_cues = Vec::new();

    let sources: Vec<Source> = match source {
        Source::All => vec![Source::Safari, Source::Notes, Source::Messages],
        other => vec![other],
    };

    let mut succeeded_sources: Vec<(&str, Vec<cueward_core::Cue>)> = Vec::new();

    for src in &sources {
        let name = source_name(src);
        let result = match src {
            Source::Safari => adapter.capture_browser_history(since_dt),
            Source::Notes => adapter.capture_notes(since_dt),
            Source::Messages => adapter.capture_messages(since_dt),
            Source::All => unreachable!(),
        };

        match result {
            Ok(cues) => succeeded_sources.push((name, cues)),
            Err(e) => eprintln!("warning: {e}"),
        }
    }

    let mut state = State::load();
    for (name, cues) in &succeeded_sources {
        if let Some(max_ts) = cues.iter().map(|c| c.timestamp).max() {
            state.set_watermark(name, max_ts);
        }
        all_cues.extend(cues.iter().cloned());
    }
    if let Err(e) = state.save() {
        eprintln!("warning: failed to save state: {e}");
    }

    match inbox::save(&all_cues) {
        Ok(path) => eprintln!("saved to {}", path.display()),
        Err(e) => eprintln!("warning: failed to save inbox: {e}"),
    }

    let json = serde_json::to_string_pretty(&all_cues).unwrap();
    print_external("capture", &json);
    eprintln!("captured {} cues", all_cues.len());
}
