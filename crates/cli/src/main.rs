use clap::{Parser, Subcommand, ValueEnum};

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

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Capture { source, since } => {
            let source_name = match source {
                Source::Safari => "safari",
                Source::Notes => "notes",
                Source::Messages => "messages",
                Source::All => "all",
            };
            eprintln!("capture: source={source_name}, since={since}");
            eprintln!("(not yet implemented)");
        }
        Command::Triage => {
            eprintln!("triage: not yet implemented");
        }
    }
}
