use std::process;

use chrono::Utc;
use clap::{Parser, Subcommand, ValueEnum};

use cueward_adapter_macos::MacosAdapter;
use cueward_core::{PlatformAdapter, State};

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
                let name = match src {
                    Source::Safari => "safari",
                    Source::Notes => "notes",
                    Source::Messages => "messages",
                    Source::All => unreachable!(),
                };

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

            let json = serde_json::to_string_pretty(&all_cues).unwrap();
            println!("{json}");

            eprintln!("captured {} cues", all_cues.len());
        }
        Command::Triage => {
            eprintln!("triage: not yet implemented");
            process::exit(1);
        }
    }
}
