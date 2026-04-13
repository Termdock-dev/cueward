use clap::{Parser, Subcommand, ValueEnum};

pub(crate) mod calendar;
pub(crate) mod capture;
pub(crate) mod clipboard;
pub(crate) mod helpers;
pub(crate) mod notes;
pub(crate) mod ocr;
pub(crate) mod quick_notes;
pub(crate) mod reminders;
pub(crate) mod safari;
pub(crate) mod safari_ai;
pub(crate) mod safari_bookmarks;
pub(crate) mod screenshot;
pub(crate) mod search;
pub(crate) mod send;
pub(crate) mod triage;
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
    Capture {
        #[arg(long, default_value = "all")]
        source: Source,
        #[arg(long, default_value = "24h")]
        since: String,
    },
    Triage,
    Search {
        query: String,
        #[arg(long, default_value = "10")]
        limit: usize,
    },
    Send {
        #[arg(long)]
        title: String,
        #[arg(long)]
        body: Option<String>,
        #[arg(long, default_value = "Cueward")]
        folder: String,
        #[arg(long)]
        notify: bool,
    },
    Plan {
        #[arg(long)]
        title: String,
        #[arg(long, default_value = "")]
        notes: String,
        #[arg(long, default_value = "Cueward")]
        list: String,
    },
    Reminders {
        #[command(subcommand)]
        action: RemindersAction,
    },
    Ocr {
        path: String,
    },
    Safari {
        #[command(subcommand)]
        action: SafariAction,
    },
    Notes {
        #[command(subcommand)]
        action: NotesAction,
    },
    QuickNotes {
        #[command(subcommand)]
        action: QuickNotesAction,
    },
    Calendar {
        #[command(subcommand)]
        action: CalendarAction,
    },
    Screenshot {
        #[arg(long)]
        ocr: bool,
        #[arg(long)]
        output: Option<String>,
        #[arg(long)]
        display: Option<u32>,
    },
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
