use clap::{Parser, Subcommand, ValueEnum};

pub(crate) mod calendar;
pub(crate) mod capture;
pub(crate) mod clipboard;
pub(crate) mod helpers;
pub(crate) mod notes;
pub(crate) mod ocr;
pub(crate) mod quick_notes;
pub(crate) mod reddit;
pub(crate) mod reminders;
pub(crate) mod safari;
pub(crate) mod safari_ai;
pub(crate) mod safari_bookmarks;
pub(crate) mod screenshot;
pub(crate) mod search;
pub(crate) mod send;
pub(crate) mod triage;
#[cfg(test)]
mod notes_tests;
#[cfg(test)]
mod reddit_tests;
#[cfg(test)]
mod safari_ai_tests;
#[cfg(test)]
mod safari_bookmarks_tests;
#[cfg(test)]
mod safari_tests;

pub(crate) use calendar::CalendarAction;
pub(crate) use clipboard::ClipboardAction;
pub(crate) use notes::NotesAction;
pub(crate) use quick_notes::QuickNotesAction;
pub(crate) use reddit::RedditAction;
pub(crate) use reminders::RemindersAction;
pub(crate) use safari::SafariAction;

#[derive(Parser)]
#[command(
    name = "cueward",
    about = "Capture and triage your scattered knowledge"
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand)]
pub(crate) enum Command {
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
    /// Send a digest note or system notification
    Send {
        /// Note title
        #[arg(long)]
        title: String,
        /// Note body (read from stdin if not provided)
        #[arg(long)]
        body: Option<String>,
        /// Target Notes folder
        #[arg(long, default_value = "Cueward")]
        folder: String,
        /// Also send a macOS notification
        #[arg(long)]
        notify: bool,
    },
    /// Create a reminder or calendar event
    Plan {
        /// Reminder/event title
        #[arg(long)]
        title: String,
        /// Notes or description
        #[arg(long, default_value = "")]
        notes: String,
        /// Reminders list name
        #[arg(long, default_value = "Cueward")]
        list: String,
    },
    /// Read Apple Reminders
    Reminders {
        #[command(subcommand)]
        action: RemindersAction,
    },
    /// Read Reddit via public JSON endpoints
    Reddit {
        #[command(subcommand)]
        action: RedditAction,
    },
    /// Extract text from images or PDFs via Vision OCR
    Ocr {
        /// Path to image or PDF file
        path: String,
    },
    /// Read current Safari tabs and active tab
    Safari {
        #[command(subcommand)]
        action: SafariAction,
    },
    /// Manage Apple Notes (update, delete, move)
    Notes {
        #[command(subcommand)]
        action: NotesAction,
    },
    /// Manage Quick Notes (快速備忘錄)
    QuickNotes {
        #[command(subcommand)]
        action: QuickNotesAction,
    },
    /// Query and manage Apple Calendar events
    Calendar {
        #[command(subcommand)]
        action: CalendarAction,
    },
    /// Capture a screenshot of the screen
    Screenshot {
        /// Also run OCR on the captured image
        #[arg(long)]
        ocr: bool,
        /// Output path (default: ~/.cueward/cache/screenshots/<timestamp>.png)
        #[arg(long)]
        output: Option<String>,
        /// Display number (1 = main, 2 = secondary, 3 = third)
        #[arg(long)]
        display: Option<u32>,
    },
    /// Read or write the system clipboard
    Clipboard {
        #[command(subcommand)]
        action: ClipboardAction,
    },
}

#[derive(Clone, ValueEnum)]
pub(crate) enum Source {
    Safari,
    Notes,
    Messages,
    All,
}

pub(crate) fn dispatch(command: Command) {
    match command {
        Command::Capture { source, since } => capture::dispatch(source, since),
        Command::Triage => triage::dispatch(),
        Command::Search { query, limit } => search::dispatch(query, limit),
        Command::Send {
            title,
            body,
            folder,
            notify,
        } => send::dispatch(title, body, folder, notify),
        Command::Plan { title, notes, list } => reminders::dispatch_plan(title, notes, list),
        Command::Reminders { action } => reminders::dispatch(action),
        Command::Reddit { action } => reddit::dispatch(action),
        Command::Ocr { path } => ocr::dispatch(path),
        Command::Safari { action } => safari::dispatch(action),
        Command::Notes { action } => notes::dispatch(action),
        Command::QuickNotes { action } => quick_notes::dispatch(action),
        Command::Calendar { action } => calendar::dispatch(action),
        Command::Screenshot {
            ocr,
            output,
            display,
        } => screenshot::dispatch(ocr, output, display),
        Command::Clipboard { action } => clipboard::dispatch(action),
    }
}
