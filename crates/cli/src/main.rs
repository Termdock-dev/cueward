use std::process;

use chrono::Utc;
use clap::{Parser, Subcommand, ValueEnum};

use cueward_adapter_macos::MacosAdapter;
use cueward_core::{inbox, CueIndex, PlatformAdapter, State, Tagger};

#[derive(Parser)]
#[command(name = "cueward", about = "Capture and triage your scattered knowledge")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Capture knowledge fragments from local sources
    Capture {
        /// Data source to capture from
        #[arg(long, default_value = "all")]
        source: Source,

        /// Time window (e.g. "24h", "7d")
        #[arg(long, default_value = "24h")]
        since: String,
    },

    /// Categorize, tag, and index captured cues
    Triage,

    /// Search indexed cues
    Search {
        /// Search query
        query: String,

        /// Max results
        #[arg(long, default_value = "10")]
        limit: usize,
    },
}

#[derive(Clone, ValueEnum)]
enum Source {
    Safari,
    Notes,
    Messages,
    All,
}

fn parse_duration(s: &str) -> Option<chrono::Duration> {
    let s = s.trim();
    if let Some(hours) = s.strip_suffix('h') {
        hours.parse().ok().map(chrono::Duration::hours)
    } else if let Some(days) = s.strip_suffix('d') {
        days.parse().ok().map(chrono::Duration::days)
    } else if let Some(mins) = s.strip_suffix('m') {
        mins.parse().ok().map(chrono::Duration::minutes)
    } else {
        None
    }
}

fn source_name(src: &Source) -> &'static str {
    match src {
        Source::Safari => "safari",
        Source::Notes => "notes",
        Source::Messages => "messages",
        Source::All => unreachable!(),
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Capture { source, since } => {
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

            // Update watermark only for successful sources, using max captured timestamp
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

            // Save to inbox for triage
            match inbox::save(&all_cues) {
                Ok(path) => eprintln!("saved to {}", path.display()),
                Err(e) => eprintln!("warning: failed to save inbox: {e}"),
            }

            let json = serde_json::to_string_pretty(&all_cues).unwrap();
            println!("{json}");

            eprintln!("captured {} cues", all_cues.len());
        }

        Command::Triage => {
            let batches = match inbox::load_all() {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("error: failed to read inbox: {e}");
                    process::exit(1);
                }
            };

            if batches.is_empty() {
                eprintln!("inbox is empty. run `cueward capture` first.");
                return;
            }

            let tagger = Tagger::load();
            let idx = match CueIndex::open_or_create() {
                Ok(i) => i,
                Err(e) => {
                    eprintln!("error: failed to open index: {e}");
                    process::exit(1);
                }
            };

            let mut total = 0;
            for (path, mut cues) in batches {
                if let Some(t) = &tagger {
                    t.tag_all(&mut cues);
                }

                match idx.add_cues(&cues) {
                    Ok(n) => total += n,
                    Err(e) => {
                        eprintln!("error: failed to index: {e}");
                        process::exit(1);
                    }
                }

                if let Err(e) = inbox::mark_done(&path) {
                    eprintln!("warning: failed to move {}: {e}", path.display());
                }
            }

            if tagger.is_some() {
                eprintln!("auto-tagged with ~/.cueward/tags.toml");
            } else {
                eprintln!("no tags.toml found, skipping auto-tag");
            }
            eprintln!("indexed {total} cues");
        }

        Command::Search { query, limit } => {
            let idx = match CueIndex::open_or_create() {
                Ok(i) => i,
                Err(e) => {
                    eprintln!("error: failed to open index: {e}");
                    process::exit(1);
                }
            };

            match idx.search(&query, limit) {
                Ok(results) => {
                    if results.is_empty() {
                        eprintln!("no results found");
                    } else {
                        for r in &results {
                            println!("{r}");
                        }
                        eprintln!("{} results", results.len());
                    }
                }
                Err(e) => {
                    eprintln!("error: search failed: {e}");
                    process::exit(1);
                }
            }
        }
    }
}
