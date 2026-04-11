use std::process;

use chrono::{DateTime, Local, TimeZone, Utc};
use clap::{Parser, Subcommand, ValueEnum};

use cueward_adapter_macos::MacosAdapter;
use cueward_core::{CueIndex, PlatformAdapter, State, Tagger, inbox};

#[derive(Parser)]
#[command(
    name = "cueward",
    about = "Capture and triage your scattered knowledge"
)]
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

#[derive(Subcommand)]
enum ClipboardAction {
    /// Read clipboard content (text or image)
    Get {
        /// Save image to this path instead of default cache dir
        #[arg(long)]
        save_image: Option<String>,
    },

    /// Write text to clipboard
    Set {
        /// Text to copy to clipboard
        text: String,
    },
}

#[derive(Subcommand)]
enum NotesAction {
    /// Update a note's body
    Update {
        /// Note title to find
        #[arg(long)]
        title: String,

        /// New body content
        #[arg(long)]
        body: String,

        /// Folder to search in
        #[arg(long, default_value = "Cueward")]
        folder: String,
    },

    /// Delete a note
    Delete {
        /// Note title to find
        #[arg(long)]
        title: String,

        /// Folder to search in
        #[arg(long, default_value = "Cueward")]
        folder: String,
    },

    /// Move a note to a different folder
    Move {
        /// Note title to find
        #[arg(long)]
        title: String,

        /// Source folder
        #[arg(long)]
        from: String,

        /// Destination folder
        #[arg(long)]
        to: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, ValueEnum)]
enum SafariAiProvider {
    Gemini,
    Chatgpt,
}

#[derive(Clone, Debug, PartialEq, Eq, ValueEnum)]
enum GeminiMode {
    Image,
    DeepResearch,
    Video,
    Music,
}

#[derive(Subcommand)]
enum SafariAction {
    /// List all current Safari tabs
    Tabs {
        /// Filter by Safari profile name parsed from window title
        #[arg(long)]
        profile: Option<String>,
    },

    /// Show the current active tab in the front Safari window
    Active,

    /// Open a URL in a new Safari tab
    Open {
        /// URL to open
        url: String,
    },

    /// Close a tab in the front Safari window
    Close {
        /// Zero-based tab index in the front window. Defaults to the current tab.
        #[arg(long)]
        index: Option<usize>,
    },

    /// Read page content from the current active tab
    Read {
        /// Optional CSS selector to extract a specific element's text
        #[arg(long)]
        selector: Option<String>,
    },

    /// Read the full HTML source of the current active tab
    Source,

    /// Execute JavaScript in the current active tab
    Exec {
        /// JavaScript code to execute
        js_code: String,
    },

    /// Click an element in the current active tab
    Click {
        /// CSS selector
        selector: String,
    },

    /// Fill an element in the current active tab
    Fill {
        /// CSS selector
        selector: String,
        /// Text to fill
        text: String,
    },

    /// Wait for an element to appear in the current active tab
    Wait {
        /// CSS selector
        selector: String,
        /// Timeout in seconds
        #[arg(long, default_value = "30")]
        timeout: u64,
    },

    /// Run a high-level Safari AI workflow
    Ai {
        /// AI provider to target
        #[arg(long)]
        provider: SafariAiProvider,
        /// Optional Gemini mode to switch into before interaction
        #[arg(long)]
        mode: Option<GeminiMode>,
        /// Prompt to send in a later workflow stage
        #[arg(long)]
        prompt: Option<String>,
    },
}

#[derive(Subcommand)]
enum QuickNotesAction {
    /// List all Quick Notes
    List,

    /// Create a new Quick Note
    Create {
        /// Note title
        #[arg(long)]
        title: String,

        /// Note body
        #[arg(long)]
        body: String,
    },

    /// Update a Quick Note's body
    Update {
        /// Note title to find
        #[arg(long)]
        title: String,

        /// New body content
        #[arg(long)]
        body: String,
    },

    /// Delete a Quick Note
    Delete {
        /// Note title to find
        #[arg(long)]
        title: String,
    },

    /// Archive a Quick Note into a regular folder and remove it from Quick Notes
    Archive {
        /// Note title to find. Must be unique among Quick Notes.
        #[arg(long)]
        title: String,

        /// Destination folder for the archived regular note
        #[arg(long)]
        to: String,
    },
}

#[derive(Subcommand)]
enum RemindersAction {
    /// List reminders, optionally filtered by list name
    List {
        /// Filter by reminders list name
        #[arg(long)]
        list: Option<String>,
    },

    /// List reminders due today
    Today,
}

#[derive(Subcommand)]
enum CalendarAction {
    /// List events in a time range (default: next 24h)
    List {
        /// Start datetime (RFC3339 or "YYYY-MM-DD HH:MM")
        #[arg(long)]
        from: Option<String>,

        /// End datetime (RFC3339 or "YYYY-MM-DD HH:MM")
        #[arg(long)]
        to: Option<String>,

        /// Filter by calendar name
        #[arg(long)]
        calendar: Option<String>,
    },

    /// List today's events (00:00 to 23:59)
    Today {
        /// Filter by calendar name
        #[arg(long)]
        calendar: Option<String>,
    },

    /// Create a calendar event
    Create {
        /// Event title
        #[arg(long)]
        title: String,

        /// Start datetime (RFC3339 or "YYYY-MM-DD HH:MM")
        #[arg(long)]
        start: String,

        /// End datetime (RFC3339 or "YYYY-MM-DD HH:MM")
        #[arg(long)]
        end: String,

        /// Calendar name (uses default calendar if omitted)
        #[arg(long)]
        calendar: Option<String>,

        /// Notes/description
        #[arg(long)]
        notes: Option<String>,

        /// Location
        #[arg(long)]
        location: Option<String>,
    },

    /// Delete a calendar event by title and start datetime
    Delete {
        /// Event title
        #[arg(long)]
        title: String,

        /// Start datetime (RFC3339 or "YYYY-MM-DD HH:MM")
        #[arg(long)]
        start: String,

        /// Calendar name
        #[arg(long)]
        calendar: String,
    },
}

#[derive(Clone, ValueEnum)]
enum Source {
    Safari,
    Notes,
    Messages,
    All,
}

fn parse_datetime(s: &str) -> Option<DateTime<Local>> {
    use chrono::NaiveDateTime;

    // Try RFC 3339 first
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Local));
    }
    // Try "YYYY-MM-DD HH:MM:SS"
    if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        if let Some(dt) = Local
            .from_local_datetime(&ndt)
            .single()
            .or_else(|| Local.from_local_datetime(&ndt).earliest())
            .or_else(|| Local.from_local_datetime(&ndt).latest())
        {
            return Some(dt);
        }
    }
    // Try "YYYY-MM-DD HH:MM"
    if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M") {
        if let Some(dt) = Local
            .from_local_datetime(&ndt)
            .single()
            .or_else(|| Local.from_local_datetime(&ndt).earliest())
            .or_else(|| Local.from_local_datetime(&ndt).latest())
        {
            return Some(dt);
        }
    }
    None
}

fn parse_datetime_arg(label: &str, value: &str) -> Result<DateTime<Local>, String> {
    parse_datetime(value).ok_or_else(|| format!("error: invalid {label} datetime '{value}'"))
}

fn parse_required_datetime_arg(
    label: &str,
    value: Option<&str>,
) -> Result<DateTime<Local>, String> {
    match value {
        Some(value) => parse_datetime_arg(label, value),
        None => Err(format!("error: missing {label} datetime")),
    }
}

fn validate_optional_output_path(label: &str, value: Option<&str>) -> Result<(), String> {
    if let Some(path) = value {
        if std::path::Path::new(path)
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(format!(
                "error: {label} path must not contain parent directory components"
            ));
        }
    }
    Ok(())
}

fn local_day_bounds(now: DateTime<Local>) -> Result<(DateTime<Local>, DateTime<Local>), String> {
    let from = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .and_then(|dt| Local.from_local_datetime(&dt).single())
        .ok_or_else(|| "error: could not determine start of today".to_string())?;
    let to = now
        .date_naive()
        .and_hms_opt(23, 59, 59)
        .and_then(|dt| Local.from_local_datetime(&dt).single())
        .ok_or_else(|| "error: could not determine end of today".to_string())?;
    Ok((from, to))
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

fn to_adapter_gemini_mode(mode: GeminiMode) -> cueward_adapter_macos::safari::GeminiMode {
    match mode {
        GeminiMode::Image => cueward_adapter_macos::safari::GeminiMode::Image,
        GeminiMode::DeepResearch => cueward_adapter_macos::safari::GeminiMode::DeepResearch,
        GeminiMode::Video => cueward_adapter_macos::safari::GeminiMode::Video,
        GeminiMode::Music => cueward_adapter_macos::safari::GeminiMode::Music,
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
                    eprintln!("error: failed to move {}: {e}", path.display());
                    eprintln!("aborting to prevent duplicate indexing on next triage run");
                    process::exit(1);
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

        Command::Send {
            title,
            body,
            folder,
            notify,
        } => {
            let body = body.unwrap_or_else(|| {
                use std::io::Read;
                let mut buf = String::new();
                std::io::stdin()
                    .read_to_string(&mut buf)
                    .unwrap_or_default();
                buf
            });

            match cueward_adapter_macos::send::create_note(&title, &body, &folder) {
                Ok(()) => eprintln!("note created in {folder}"),
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }

            if notify {
                let flat = body.replace('\n', " ");
                let preview = if flat.chars().count() > 100 {
                    let truncated: String = flat.chars().take(100).collect();
                    format!("{truncated}...")
                } else {
                    flat
                };
                if let Err(e) = cueward_adapter_macos::send::notify(&title, &preview) {
                    eprintln!("warning: notification failed: {e}");
                }
            }
        }

        Command::Plan { title, notes, list } => {
            match cueward_adapter_macos::plan::create_reminder(&title, &notes, &list) {
                Ok(()) => eprintln!("reminder created in {list}"),
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }

        Command::Reminders { action } => match action {
            RemindersAction::List { list } => {
                match cueward_adapter_macos::reminders::list(list.as_deref()) {
                    Ok(reminders) => {
                        println!("{}", serde_json::to_string_pretty(&reminders).unwrap());
                        eprintln!("{} reminder(s)", reminders.len());
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            }
            RemindersAction::Today => match cueward_adapter_macos::reminders::today() {
                Ok(reminders) => {
                    println!("{}", serde_json::to_string_pretty(&reminders).unwrap());
                    eprintln!("{} reminder(s) due today", reminders.len());
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            },
        },

        Command::Ocr { path } => match cueward_adapter_macos::ocr::capture(&path) {
            Ok(cues) => {
                let json = serde_json::to_string_pretty(&cues).unwrap();
                println!("{json}");
                eprintln!("extracted {} cues", cues.len());
            }
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        },

        Command::Safari { action } => match action {
            SafariAction::Tabs { profile } => {
                match cueward_adapter_macos::safari::tabs(profile.as_deref()) {
                    Ok(tabs) => {
                        println!("{}", serde_json::to_string_pretty(&tabs).unwrap());
                        eprintln!("{} tab(s)", tabs.len());
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            }
            SafariAction::Active => match cueward_adapter_macos::safari::active() {
                Ok(tab) => {
                    println!("{}", serde_json::to_string_pretty(&tab).unwrap());
                    if tab.is_some() {
                        eprintln!("active tab");
                    } else {
                        eprintln!("no Safari window");
                    }
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            },
            SafariAction::Open { url } => match cueward_adapter_macos::safari::open(&url) {
                Ok(tab) => {
                    println!("{}", serde_json::to_string_pretty(&tab).unwrap());
                    if tab.is_some() {
                        eprintln!("opened tab");
                    } else {
                        eprintln!("no Safari window");
                    }
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            },
            SafariAction::Close { index } => match cueward_adapter_macos::safari::close(index) {
                Ok(result) => {
                    println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    if result.closed {
                        eprintln!("tab closed");
                    } else {
                        eprintln!("no Safari window");
                    }
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            },
            SafariAction::Read { selector } => {
                match cueward_adapter_macos::safari::read(selector.as_deref()) {
                    Ok(result) => {
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                        eprintln!("read page content");
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            }
            SafariAction::Source => match cueward_adapter_macos::safari::source() {
                Ok(result) => {
                    println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    eprintln!("read page source");
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            },
            SafariAction::Exec { js_code } => match cueward_adapter_macos::safari::exec(&js_code) {
                Ok(result) => {
                    println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    eprintln!("executed javascript");
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            },
            SafariAction::Click { selector } => {
                match cueward_adapter_macos::safari::click(&selector) {
                    Ok(result) => {
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                        eprintln!("clicked element");
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            }
            SafariAction::Fill { selector, text } => {
                match cueward_adapter_macos::safari::fill(&selector, &text) {
                    Ok(result) => {
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                        eprintln!("filled element");
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            }
            SafariAction::Wait { selector, timeout } => {
                match cueward_adapter_macos::safari::wait(&selector, timeout) {
                    Ok(result) => {
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                        eprintln!("selector found");
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            }
            SafariAction::Ai {
                provider,
                mode,
                prompt,
            } => match provider {
                SafariAiProvider::Gemini => {
                    if let Some(mode) = mode {
                        match cueward_adapter_macos::safari::prepare_gemini_mode(
                            to_adapter_gemini_mode(mode),
                        ) {
                            Ok(result) => {
                                println!("{}", serde_json::to_string_pretty(&result).unwrap());
                                eprintln!("gemini mode ready");
                            }
                            Err(e) => {
                                eprintln!("error: {e}");
                                process::exit(1);
                            }
                        }
                    } else {
                        let Some(prompt) = prompt.as_deref() else {
                            eprintln!("error: --prompt is required for Gemini chat workflow");
                            process::exit(1);
                        };

                        match cueward_adapter_macos::safari::send_gemini_prompt(prompt) {
                            Ok(result) => {
                                println!("{}", serde_json::to_string_pretty(&result).unwrap());
                                eprintln!("gemini response ready");
                            }
                            Err(e) => {
                                eprintln!("error: {e}");
                                process::exit(1);
                            }
                        }
                    }
                }
                SafariAiProvider::Chatgpt => {
                    eprintln!("error: ChatGPT Safari AI workflow not implemented yet");
                    process::exit(1);
                }
            },
        },

        Command::Notes { action } => match action {
            NotesAction::Update {
                title,
                body,
                folder,
            } => match cueward_adapter_macos::send::update_note(&title, &body, &folder) {
                Ok(()) => eprintln!("note updated: {title}"),
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            },
            NotesAction::Delete { title, folder } => {
                match cueward_adapter_macos::send::delete_note(&title, &folder) {
                    Ok(()) => eprintln!("note deleted: {title}"),
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            }
            NotesAction::Move { title, from, to } => {
                match cueward_adapter_macos::send::move_note(&title, &from, &to) {
                    Ok(()) => eprintln!("note moved: {title} ({from} -> {to})"),
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            }
        },

        Command::QuickNotes { action } => match action {
            QuickNotesAction::List => match cueward_adapter_macos::quick_notes::list() {
                Ok(notes) => {
                    if notes.is_empty() {
                        eprintln!("no quick notes found");
                    } else {
                        let count = notes.len();
                        println!("{}", serde_json::to_string_pretty(&notes).unwrap());
                        eprintln!("{count} quick note(s)");
                    }
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            },
            QuickNotesAction::Create { title, body } => {
                match cueward_adapter_macos::quick_notes::create(&title, &body) {
                    Ok(()) => eprintln!("quick note created: {title}"),
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            }
            QuickNotesAction::Update { title, body } => {
                match cueward_adapter_macos::quick_notes::update(&title, &body) {
                    Ok(()) => eprintln!("quick note updated: {title}"),
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            }
            QuickNotesAction::Delete { title } => {
                match cueward_adapter_macos::quick_notes::delete(&title) {
                    Ok(()) => eprintln!("quick note deleted: {title}"),
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            }
            QuickNotesAction::Archive { title, to } => {
                match cueward_adapter_macos::quick_notes::archive(&title, &to) {
                    Ok(()) => eprintln!("quick note archived: {title} -> {to}"),
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            }
        },

        Command::Screenshot {
            ocr,
            output,
            display,
        } => {
            if let Err(err) = validate_optional_output_path("--output", output.as_deref()) {
                eprintln!("{err}");
                process::exit(1);
            }
            match cueward_adapter_macos::screenshot::capture(ocr, output.as_deref(), display) {
                Ok(result) => {
                    println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    eprintln!("screenshot saved to {}", result.path);
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }

        Command::Clipboard { action } => match action {
            ClipboardAction::Get { save_image } => {
                if let Err(err) =
                    validate_optional_output_path("--save-image", save_image.as_deref())
                {
                    eprintln!("{err}");
                    process::exit(1);
                }
                match cueward_adapter_macos::clipboard::get(save_image.as_deref()) {
                    Ok(content) => {
                        println!("{}", serde_json::to_string_pretty(&content).unwrap());
                        match content.content_type.as_str() {
                            "image" => eprintln!(
                                "clipboard image saved to {}",
                                content.path.unwrap_or_default()
                            ),
                            _ => eprintln!("clipboard text read"),
                        }
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            }
            ClipboardAction::Set { text } => match cueward_adapter_macos::clipboard::set(&text) {
                Ok(()) => eprintln!("copied to clipboard"),
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            },
        },

        Command::Calendar { action } => match action {
            CalendarAction::Today { calendar } => {
                let now = Local::now();
                let (from, to) = match local_day_bounds(now) {
                    Ok(bounds) => bounds,
                    Err(err) => {
                        eprintln!("{err}");
                        process::exit(1);
                    }
                };
                match cueward_adapter_macos::calendar::list_events(from, to, calendar.as_deref()) {
                    Ok(events) => {
                        println!("{}", serde_json::to_string_pretty(&events).unwrap());
                        eprintln!("{} event(s)", events.len());
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            }

            CalendarAction::List { from, to, calendar } => {
                let now = Local::now();
                let from_dt = match from.as_deref() {
                    Some(value) => match parse_required_datetime_arg("--from", Some(value)) {
                        Ok(dt) => dt,
                        Err(err) => {
                            eprintln!("{err}");
                            process::exit(1);
                        }
                    },
                    None => now,
                };
                let to_dt = match to.as_deref() {
                    Some(value) => match parse_required_datetime_arg("--to", Some(value)) {
                        Ok(dt) => dt,
                        Err(err) => {
                            eprintln!("{err}");
                            process::exit(1);
                        }
                    },
                    None => from_dt + chrono::Duration::hours(24),
                };
                match cueward_adapter_macos::calendar::list_events(
                    from_dt,
                    to_dt,
                    calendar.as_deref(),
                ) {
                    Ok(events) => {
                        println!("{}", serde_json::to_string_pretty(&events).unwrap());
                        eprintln!("{} event(s)", events.len());
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            }

            CalendarAction::Create {
                title,
                start,
                end,
                calendar,
                notes,
                location,
            } => {
                let start_dt = match parse_datetime_arg("start", &start) {
                    Ok(dt) => dt,
                    Err(err) => {
                        eprintln!("{err}");
                        process::exit(1);
                    }
                };
                let end_dt = match parse_datetime_arg("end", &end) {
                    Ok(dt) => dt,
                    Err(err) => {
                        eprintln!("{err}");
                        process::exit(1);
                    }
                };
                match cueward_adapter_macos::calendar::create_event(
                    &title,
                    start_dt,
                    end_dt,
                    calendar.as_deref(),
                    notes.as_deref(),
                    location.as_deref(),
                ) {
                    Ok(()) => eprintln!("event created: {title}"),
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            }

            CalendarAction::Delete {
                title,
                start,
                calendar,
            } => {
                let start_dt = match parse_datetime_arg("start", &start) {
                    Ok(dt) => dt,
                    Err(err) => {
                        eprintln!("{err}");
                        process::exit(1);
                    }
                };
                match cueward_adapter_macos::calendar::delete_event(&title, start_dt, &calendar) {
                    Ok(()) => eprintln!("event deleted: {title}"),
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use chrono::Timelike;
    use chrono::{Local, TimeZone};
    use clap::Parser;

    use super::{
        Cli, Command, GeminiMode, SafariAction, SafariAiProvider, local_day_bounds,
        validate_optional_output_path,
    };

    #[test]
    fn validate_optional_output_path_rejects_parent_components() {
        let result = validate_optional_output_path("--output", Some("../secret.png"));

        assert_eq!(
            result,
            Err("error: --output path must not contain parent directory components".to_string())
        );
    }

    #[test]
    fn local_day_bounds_covers_full_day() {
        let now = Local
            .with_ymd_and_hms(2026, 4, 11, 10, 30, 0)
            .single()
            .expect("local dt");

        let (from, to) = local_day_bounds(now).expect("bounds");

        assert_eq!(from.hour(), 0);
        assert_eq!(from.minute(), 0);
        assert_eq!(from.second(), 0);
        assert_eq!(to.hour(), 23);
        assert_eq!(to.minute(), 59);
        assert_eq!(to.second(), 59);
    }

    #[test]
    fn parse_datetime_accepts_ambiguous_local_time() {
        let parsed = super::parse_datetime("2026-11-01 01:30");

        assert!(parsed.is_some());
    }

    #[test]
    fn cli_parses_gemini_mode_switch_command() {
        let cli = Cli::try_parse_from([
            "cueward",
            "safari",
            "ai",
            "--provider",
            "gemini",
            "--mode",
            "deep-research",
            "--prompt",
            "研究議題",
        ])
        .expect("parse safari ai command");

        match cli.command {
            Command::Safari {
                action:
                    SafariAction::Ai {
                        provider,
                        mode,
                        prompt,
                    },
            } => {
                assert_eq!(provider, SafariAiProvider::Gemini);
                assert_eq!(mode, Some(GeminiMode::DeepResearch));
                assert_eq!(prompt, Some("研究議題".to_string()));
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn cli_parses_gemini_chat_command() {
        let cli = Cli::try_parse_from([
            "cueward",
            "safari",
            "ai",
            "--provider",
            "gemini",
            "--prompt",
            "哈囉",
        ])
        .expect("parse gemini chat command");

        match cli.command {
            Command::Safari {
                action:
                    SafariAction::Ai {
                        provider,
                        mode,
                        prompt,
                    },
            } => {
                assert_eq!(provider, SafariAiProvider::Gemini);
                assert_eq!(mode, None);
                assert_eq!(prompt, Some("哈囉".to_string()));
            }
            _ => panic!("unexpected command"),
        }
    }
}
