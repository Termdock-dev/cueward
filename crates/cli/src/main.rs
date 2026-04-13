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
    Grok,
    Threads,
    X,
}

#[derive(Clone, Debug, PartialEq, Eq, ValueEnum)]
enum GeminiMode {
    Image,
    DeepResearch,
    Video,
    Music,
}

#[derive(Subcommand)]
enum SafariAiAction {
    /// Send a prompt to the AI provider
    Prompt {
        /// Prompt text
        #[arg(long)]
        prompt: String,
        /// Optional mode (e.g. deep-research, image, video, music)
        #[arg(long)]
        mode: Option<GeminiMode>,
        /// Automatically confirm (e.g. Deep Research plan)
        #[arg(long, default_value_t = false)]
        auto_confirm: bool,
    },
    /// Switch to a specific mode without sending a prompt
    Mode {
        /// Mode to switch into
        mode: GeminiMode,
    },
    /// List conversations from the sidebar
    List,
    /// Read a conversation's text content by URL
    Read {
        /// Conversation URL
        url: String,
    },
    /// Poll an in-progress workflow (e.g. Deep Research)
    Poll {
        /// Timeout in seconds
        #[arg(long, default_value = "900")]
        timeout: u64,
    },
    /// Save AI-generated images as PNG files
    SaveImages {
        /// Conversation URL
        url: String,
        /// Output directory
        #[arg(long, default_value = ".")]
        output: String,
    },
    /// Download media (video/music) via browser
    SaveMedia {
        /// Conversation URL
        url: String,
    },
}

#[derive(Subcommand)]
enum SafariBookmarksAction {
    /// List bookmark/folder items under the bookmark root or a specific folder path
    List {
        /// Restrict bookmarks operations to a Safari profile folder at the root
        #[arg(long)]
        profile: Option<String>,
        /// Optional folder path such as Work/AI Tools
        #[arg(long)]
        folder: Option<String>,
    },

    /// Search bookmarks recursively from the root or a specific folder path
    Search {
        /// Query string to match against bookmark title or URL
        query: String,
        /// Restrict bookmarks operations to a Safari profile folder at the root
        #[arg(long)]
        profile: Option<String>,
        /// Optional folder path such as Work/AI Tools
        #[arg(long)]
        folder: Option<String>,
    },

    /// Add a bookmark under the root or a specific folder path
    Add {
        /// Bookmark title
        #[arg(long)]
        title: String,
        /// Bookmark URL
        #[arg(long)]
        url: String,
        /// Restrict bookmarks operations to a Safari profile folder at the root
        #[arg(long)]
        profile: Option<String>,
        /// Optional folder path such as Work/AI Tools
        #[arg(long)]
        folder: Option<String>,
    },

    /// Delete a bookmark by exact title + URL under the root or a specific folder path
    Delete {
        /// Bookmark title
        #[arg(long)]
        title: String,
        /// Bookmark URL
        #[arg(long)]
        url: String,
        /// Restrict bookmarks operations to a Safari profile folder at the root
        #[arg(long)]
        profile: Option<String>,
        /// Optional folder path such as Work/AI Tools
        #[arg(long)]
        folder: Option<String>,
    },
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
    Active {
        /// Restrict operations to a Safari profile name parsed from the window title
        #[arg(long)]
        profile: Option<String>,
    },

    /// Open a URL in a new Safari tab
    Open {
        /// URL to open
        url: String,
        /// Restrict operations to a Safari profile name parsed from the window title
        #[arg(long)]
        profile: Option<String>,
    },

    /// Close a tab in the front Safari window
    Close {
        /// Zero-based tab index in the front window. Defaults to the current tab.
        #[arg(long)]
        index: Option<usize>,
    },

    /// Scroll the current page
    Scroll {
        /// Direction: up, down, top, bottom
        direction: String,
        /// Pixels to scroll (default 500, ignored for top/bottom)
        #[arg(long)]
        amount: Option<i64>,
        /// Restrict operations to a Safari profile
        #[arg(long)]
        profile: Option<String>,
        /// Target a specific tab by index or URL/title substring
        #[arg(long)]
        tab: Option<String>,
    },

    /// Scroll repeatedly and return only newly loaded content
    ScrollAndRead {
        /// Number of scroll/read iterations
        #[arg(long, default_value = "1")]
        times: u64,
        /// Pixels to scroll each iteration
        #[arg(long)]
        amount: Option<i64>,
        /// Restrict operations to a Safari profile
        #[arg(long)]
        profile: Option<String>,
        /// Target a specific tab by index or URL/title substring
        #[arg(long)]
        tab: Option<String>,
        /// Optional CSS selector to scope the read area
        #[arg(long)]
        selector: Option<String>,
    },

    /// Close multiple tabs, optionally filtered by profile and/or URL pattern
    CloseTabs {
        /// Restrict to a Safari profile name
        #[arg(long)]
        profile: Option<String>,
        /// Only close tabs whose URL contains this string
        #[arg(long)]
        url: Option<String>,
    },

    /// Read page content from the current active tab
    Read {
        /// Optional CSS selector to extract a specific element's text
        #[arg(long)]
        selector: Option<String>,
        /// Restrict operations to a Safari profile name parsed from the window title
        #[arg(long)]
        profile: Option<String>,
        /// Target a specific tab by index or URL/title substring
        #[arg(long)]
        tab: Option<String>,
    },

    /// Read the full HTML source of the current active tab
    Source {
        /// Restrict operations to a Safari profile name parsed from the window title
        #[arg(long)]
        profile: Option<String>,
        /// Target a specific tab by index or URL/title substring
        #[arg(long)]
        tab: Option<String>,
    },

    /// Execute JavaScript in the current active tab
    Exec {
        /// JavaScript code to execute
        js_code: String,
        /// Restrict operations to a Safari profile name parsed from the window title
        #[arg(long)]
        profile: Option<String>,
        /// Target a specific tab by index or URL/title substring
        #[arg(long)]
        tab: Option<String>,
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

    /// Safari bookmarks workflows
    Bookmarks {
        #[command(subcommand)]
        action: SafariBookmarksAction,
    },

    /// Safari AI provider workflows
    Ai {
        /// AI provider to target
        #[arg(long)]
        provider: SafariAiProvider,
        /// Restrict operations to a Safari profile name parsed from the window title
        #[arg(long)]
        profile: Option<String>,
        #[command(subcommand)]
        action: SafariAiAction,
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

#[derive(Debug, PartialEq, Eq)]
enum GeminiAiAction {
    ModeOnly(GeminiMode),
    PromptOnly(String),
    ModeThenPrompt(GeminiMode, String),
    DeepResearchPlan(String, bool),
}

fn build_gemini_ai_action(
    mode: Option<GeminiMode>,
    prompt: Option<&str>,
    auto_confirm: bool,
) -> Result<GeminiAiAction, &'static str> {
    if auto_confirm && !matches!((&mode, prompt), (Some(GeminiMode::DeepResearch), Some(_))) {
        return Err("--auto-confirm requires --mode deep-research and --prompt");
    }

    match (mode, prompt) {
        (Some(GeminiMode::DeepResearch), Some(prompt)) => Ok(GeminiAiAction::DeepResearchPlan(
            prompt.to_string(),
            auto_confirm,
        )),
        (Some(mode), Some(prompt)) => Ok(GeminiAiAction::ModeThenPrompt(mode, prompt.to_string())),
        (Some(mode), None) => Ok(GeminiAiAction::ModeOnly(mode)),
        (None, Some(prompt)) => Ok(GeminiAiAction::PromptOnly(prompt.to_string())),
        (None, None) => Err("--mode or --prompt is required for Gemini Safari AI workflow"),
    }
}

/// Wrap JSON output with <external> tags for LLM prompt defense.
/// Content inside the tags is treated as untrusted data, not instructions.
/// Any `</external>` in the content is escaped to prevent tag boundary bypass.
fn print_external(source: &str, json: &str) {
    let safe = json.replace("</external>", "&lt;/external&gt;");
    println!("<external source=\"cueward/{source}\">");
    println!("{safe}");
    println!("</external>");
}

fn bookmarks_target_folder(profile: Option<&str>, folder: Option<&str>) -> Option<String> {
    let profile = profile.map(str::trim).filter(|value| !value.is_empty());
    let folder = folder.map(str::trim).filter(|value| !value.is_empty());

    match (profile, folder) {
        (Some(profile), Some(folder)) => Some(format!("{profile}/{folder}")),
        (Some(profile), None) => Some(profile.to_string()),
        (None, Some(folder)) => Some(folder.to_string()),
        (None, None) => None,
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
            print_external("capture", &json);

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
                print_external("ocr", &json);
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
            SafariAction::Active { profile } => {
                match cueward_adapter_macos::safari::active(profile.as_deref()) {
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
                }
            }
            SafariAction::Open { url, profile } => {
                match cueward_adapter_macos::safari::open(&url, profile.as_deref()) {
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
                }
            }
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
            SafariAction::Scroll {
                direction,
                amount,
                profile,
                tab,
            } => {
                if let Some(ref t) = tab {
                    if let Err(e) = cueward_adapter_macos::safari::focus_tab(t, profile.as_deref())
                    {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
                match cueward_adapter_macos::safari::scroll(&direction, amount, profile.as_deref())
                {
                    Ok(result) => {
                        println!("{}", serde_json::to_string_pretty(&result).unwrap());
                        eprintln!("scrolled {direction}");
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            }
            SafariAction::ScrollAndRead {
                times,
                amount,
                profile,
                tab,
                selector,
            } => {
                if let Some(ref t) = tab {
                    if let Err(e) = cueward_adapter_macos::safari::focus_tab(t, profile.as_deref())
                    {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
                match cueward_adapter_macos::safari::scroll_and_read(
                    times,
                    amount,
                    selector.as_deref(),
                    profile.as_deref(),
                ) {
                    Ok(result) => {
                        print_external(
                            "safari/scroll-and-read",
                            &serde_json::to_string_pretty(&result).unwrap(),
                        );
                        eprintln!("scroll/read pipeline complete");
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            }
            SafariAction::CloseTabs { profile, url } => {
                match cueward_adapter_macos::safari::close_tabs(profile.as_deref(), url.as_deref())
                {
                    Ok(count) => {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&serde_json::json!({ "closed": count }))
                                .unwrap()
                        );
                        eprintln!("{count} tab(s) closed");
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            }
            SafariAction::Read {
                selector,
                profile,
                tab,
            } => {
                if let Some(ref t) = tab {
                    if let Err(e) = cueward_adapter_macos::safari::focus_tab(t, profile.as_deref())
                    {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
                match cueward_adapter_macos::safari::read(selector.as_deref(), profile.as_deref()) {
                    Ok(result) => {
                        print_external(
                            "safari/read",
                            &serde_json::to_string_pretty(&result).unwrap(),
                        );
                        eprintln!("read page content");
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            }
            SafariAction::Source { profile, tab } => {
                if let Some(ref t) = tab {
                    if let Err(e) = cueward_adapter_macos::safari::focus_tab(t, profile.as_deref())
                    {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
                match cueward_adapter_macos::safari::source(profile.as_deref()) {
                    Ok(result) => {
                        print_external(
                            "safari/source",
                            &serde_json::to_string_pretty(&result).unwrap(),
                        );
                        eprintln!("read page source");
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            }
            SafariAction::Exec {
                js_code,
                profile,
                tab,
            } => {
                if let Some(ref t) = tab {
                    if let Err(e) = cueward_adapter_macos::safari::focus_tab(t, profile.as_deref())
                    {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
                match cueward_adapter_macos::safari::exec(&js_code, profile.as_deref()) {
                    Ok(result) => {
                        print_external(
                            "safari/exec",
                            &serde_json::to_string_pretty(&result).unwrap(),
                        );
                        eprintln!("executed javascript");
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            }
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
            SafariAction::Bookmarks { action } => match action {
                SafariBookmarksAction::List { profile, folder } => {
                    let target_folder =
                        bookmarks_target_folder(profile.as_deref(), folder.as_deref());
                    match cueward_adapter_macos::bookmarks::list_bookmarks(target_folder.as_deref())
                    {
                        Ok(result) => {
                            print_external(
                                "safari/bookmarks/list",
                                &serde_json::to_string_pretty(&result).unwrap(),
                            );
                            eprintln!("listed bookmarks");
                        }
                        Err(e) => {
                            eprintln!("error: {e}");
                            process::exit(1);
                        }
                    }
                }
                SafariBookmarksAction::Search {
                    query,
                    profile,
                    folder,
                } => {
                    let target_folder =
                        bookmarks_target_folder(profile.as_deref(), folder.as_deref());
                    match cueward_adapter_macos::bookmarks::search_bookmarks(
                        &query,
                        target_folder.as_deref(),
                    ) {
                        Ok(result) => {
                            print_external(
                                "safari/bookmarks/search",
                                &serde_json::to_string_pretty(&result).unwrap(),
                            );
                            eprintln!("searched bookmarks");
                        }
                        Err(e) => {
                            eprintln!("error: {e}");
                            process::exit(1);
                        }
                    }
                }
                SafariBookmarksAction::Add {
                    title,
                    url,
                    profile,
                    folder,
                } => {
                    let target_folder =
                        bookmarks_target_folder(profile.as_deref(), folder.as_deref());
                    match cueward_adapter_macos::bookmarks::add_bookmark_cli(
                        &title,
                        &url,
                        target_folder.as_deref(),
                    ) {
                        Ok(result) => {
                            print_external(
                                "safari/bookmarks/add",
                                &serde_json::to_string_pretty(&result).unwrap(),
                            );
                            eprintln!("bookmark added");
                        }
                        Err(e) => {
                            eprintln!("error: {e}");
                            process::exit(1);
                        }
                    }
                }
                SafariBookmarksAction::Delete {
                    title,
                    url,
                    profile,
                    folder,
                } => {
                    let target_folder =
                        bookmarks_target_folder(profile.as_deref(), folder.as_deref());
                    match cueward_adapter_macos::bookmarks::delete_bookmark_cli(
                        &title,
                        &url,
                        target_folder.as_deref(),
                    ) {
                        Ok(result) => {
                            print_external(
                                "safari/bookmarks/delete",
                                &serde_json::to_string_pretty(&result).unwrap(),
                            );
                            eprintln!("bookmark deleted");
                        }
                        Err(e) => {
                            eprintln!("error: {e}");
                            process::exit(1);
                        }
                    }
                }
            },
            SafariAction::Ai {
                provider,
                profile,
                action,
            } => {
                let p = profile.as_deref();
                match provider {
                    SafariAiProvider::Gemini => {
                        match action {
                            SafariAiAction::Prompt {
                                prompt,
                                mode,
                                auto_confirm,
                            } => {
                                let gemini_action =
                                    match build_gemini_ai_action(mode, Some(&prompt), auto_confirm)
                                    {
                                        Ok(a) => a,
                                        Err(err) => {
                                            eprintln!("error: {err}");
                                            process::exit(1);
                                        }
                                    };
                                match gemini_action {
                                GeminiAiAction::PromptOnly(prompt) => {
                                    if let Err(e) = cueward_adapter_macos::safari::ensure_gemini_home(p) {
                                        eprintln!("error: {e}"); process::exit(1);
                                    }
                                    match cueward_adapter_macos::safari::send_gemini_prompt(&prompt, p) {
                                        Ok(r) => { print_external("safari/ai/gemini", &serde_json::to_string_pretty(&r).unwrap()); eprintln!("gemini response ready"); }
                                        Err(e) => { eprintln!("error: {e}"); process::exit(1); }
                                    }
                                }
                                GeminiAiAction::ModeThenPrompt(mode, prompt) => {
                                    if let Err(e) = cueward_adapter_macos::safari::prepare_gemini_mode(to_adapter_gemini_mode(mode), p) {
                                        eprintln!("error: {e}"); process::exit(1);
                                    }
                                    match cueward_adapter_macos::safari::send_gemini_prompt(&prompt, p) {
                                        Ok(r) => { print_external("safari/ai/gemini", &serde_json::to_string_pretty(&r).unwrap()); eprintln!("gemini response ready"); }
                                        Err(e) => { eprintln!("error: {e}"); process::exit(1); }
                                    }
                                }
                                GeminiAiAction::DeepResearchPlan(prompt, auto_confirm) => {
                                    match cueward_adapter_macos::safari::start_gemini_deep_research(&prompt, auto_confirm, p) {
                                        Ok(r) => { print_external("safari/ai/gemini/deep-research", &serde_json::to_string_pretty(&r).unwrap()); eprintln!("gemini deep research state ready"); }
                                        Err(e) => { eprintln!("error: {e}"); process::exit(1); }
                                    }
                                }
                                GeminiAiAction::ModeOnly(_) => unreachable!(),
                            }
                            }
                            SafariAiAction::Mode { mode } => {
                                match cueward_adapter_macos::safari::prepare_gemini_mode(
                                    to_adapter_gemini_mode(mode),
                                    p,
                                ) {
                                    Ok(r) => {
                                        println!("{}", serde_json::to_string_pretty(&r).unwrap());
                                        eprintln!("gemini mode ready");
                                    }
                                    Err(e) => {
                                        eprintln!("error: {e}");
                                        process::exit(1);
                                    }
                                }
                            }
                            SafariAiAction::List => {
                                match cueward_adapter_macos::safari::gemini_list_conversations(p) {
                                    Ok(convos) => {
                                        print_external(
                                            "safari/ai/gemini/list",
                                            &serde_json::to_string_pretty(&convos).unwrap(),
                                        );
                                        eprintln!("{} conversation(s)", convos.len());
                                    }
                                    Err(e) => {
                                        eprintln!("error: {e}");
                                        process::exit(1);
                                    }
                                }
                            }
                            SafariAiAction::Read { url } => {
                                match cueward_adapter_macos::safari::gemini_read_conversation(
                                    &url, p,
                                ) {
                                    Ok(r) => {
                                        print_external(
                                            "safari/ai/gemini/read",
                                            &serde_json::to_string_pretty(&r).unwrap(),
                                        );
                                        eprintln!("conversation read");
                                    }
                                    Err(e) => {
                                        eprintln!("error: {e}");
                                        process::exit(1);
                                    }
                                }
                            }
                            SafariAiAction::Poll { timeout } => {
                                match cueward_adapter_macos::safari::poll_gemini_deep_research(
                                    timeout, p,
                                ) {
                                    Ok(r) => {
                                        print_external(
                                            "safari/ai/gemini/poll",
                                            &serde_json::to_string_pretty(&r).unwrap(),
                                        );
                                        eprintln!("polled");
                                    }
                                    Err(e) => {
                                        eprintln!("error: {e}");
                                        process::exit(1);
                                    }
                                }
                            }
                            SafariAiAction::SaveImages { url, output } => {
                                match cueward_adapter_macos::safari::gemini_save_images(
                                    &url, &output, p,
                                ) {
                                    Ok(paths) => {
                                        println!(
                                            "{}",
                                            serde_json::to_string_pretty(&paths).unwrap()
                                        );
                                        eprintln!("{} image(s) saved", paths.len());
                                    }
                                    Err(e) => {
                                        eprintln!("error: {e}");
                                        process::exit(1);
                                    }
                                }
                            }
                            SafariAiAction::SaveMedia { url } => {
                                match cueward_adapter_macos::safari::gemini_save_media(&url, p) {
                                    Ok(r) => {
                                        println!("{}", serde_json::to_string_pretty(&r).unwrap());
                                        eprintln!("media download triggered");
                                    }
                                    Err(e) => {
                                        eprintln!("error: {e}");
                                        process::exit(1);
                                    }
                                }
                            }
                        }
                    }
                    SafariAiProvider::Chatgpt => match action {
                        SafariAiAction::Prompt {
                            prompt,
                            mode,
                            auto_confirm,
                        } => {
                            if auto_confirm {
                                eprintln!("error: ChatGPT prompt does not support --auto-confirm");
                                process::exit(1);
                            }
                            if let Err(e) = cueward_adapter_macos::safari::ensure_chatgpt_home(p) {
                                eprintln!("error: {e}");
                                process::exit(1);
                            }
                            match mode {
                                None => match cueward_adapter_macos::safari::send_chatgpt_prompt(
                                    &prompt, p,
                                ) {
                                    Ok(r) => {
                                        print_external(
                                            "safari/ai/chatgpt",
                                            &serde_json::to_string_pretty(&r).unwrap(),
                                        );
                                        eprintln!("chatgpt response ready");
                                    }
                                    Err(e) => {
                                        eprintln!("error: {e}");
                                        process::exit(1);
                                    }
                                },
                                Some(GeminiMode::Image) => {
                                    match cueward_adapter_macos::safari::send_chatgpt_image_prompt(
                                        &prompt, p,
                                    ) {
                                        Ok(r) => {
                                            print_external(
                                                "safari/ai/chatgpt/image",
                                                &serde_json::to_string_pretty(&r).unwrap(),
                                            );
                                            eprintln!("chatgpt image response ready");
                                        }
                                        Err(e) => {
                                            eprintln!("error: {e}");
                                            process::exit(1);
                                        }
                                    }
                                }
                                Some(other) => {
                                    let mode_name = other
                                        .to_possible_value()
                                        .map(|v| v.get_name().to_string())
                                        .unwrap_or_else(|| "unknown".to_string());
                                    eprintln!(
                                        "error: ChatGPT prompt does not support --mode {} yet",
                                        mode_name
                                    );
                                    process::exit(1);
                                }
                            }
                        }
                        SafariAiAction::SaveImages { url, output } => {
                            match cueward_adapter_macos::safari::chatgpt_save_images(
                                &url, &output, p,
                            ) {
                                Ok(paths) => {
                                    println!("{}", serde_json::to_string_pretty(&paths).unwrap());
                                    eprintln!("{} image(s) saved", paths.len());
                                }
                                Err(e) => {
                                    eprintln!("error: {e}");
                                    process::exit(1);
                                }
                            }
                        }
                        SafariAiAction::List => {
                            match cueward_adapter_macos::safari::chatgpt_list_conversations(p) {
                                Ok(convos) => {
                                    print_external(
                                        "safari/ai/chatgpt/list",
                                        &serde_json::to_string_pretty(&convos).unwrap(),
                                    );
                                    eprintln!("{} conversation(s)", convos.len());
                                }
                                Err(e) => {
                                    eprintln!("error: {e}");
                                    process::exit(1);
                                }
                            }
                        }
                        _ => {
                            eprintln!(
                                "error: ChatGPT currently supports prompt, list, and save-images"
                            );
                            process::exit(1);
                        }
                    },
                    SafariAiProvider::Grok => match action {
                        SafariAiAction::Prompt {
                            prompt,
                            mode,
                            auto_confirm,
                        } => {
                            if auto_confirm {
                                eprintln!("error: Grok prompt does not support --auto-confirm");
                                process::exit(1);
                            }
                            if mode.is_some() {
                                eprintln!("error: Grok prompt does not support --mode yet");
                                process::exit(1);
                            }
                            if let Err(e) = cueward_adapter_macos::safari::ensure_grok_home(p) {
                                eprintln!("error: {e}");
                                process::exit(1);
                            }
                            match cueward_adapter_macos::safari::send_grok_prompt(&prompt, p) {
                                Ok(r) => {
                                    print_external(
                                        "safari/ai/grok",
                                        &serde_json::to_string_pretty(&r).unwrap(),
                                    );
                                    eprintln!("grok response ready");
                                }
                                Err(e) => {
                                    eprintln!("error: {e}");
                                    process::exit(1);
                                }
                            }
                        }
                        SafariAiAction::List => {
                            if let Err(e) = cueward_adapter_macos::safari::ensure_grok_home(p) {
                                eprintln!("error: {e}");
                                process::exit(1);
                            }
                            match cueward_adapter_macos::safari::grok_list_conversations(p) {
                                Ok(convos) => {
                                    print_external(
                                        "safari/ai/grok/list",
                                        &serde_json::to_string_pretty(&convos).unwrap(),
                                    );
                                    eprintln!("{} conversation(s)", convos.len());
                                }
                                Err(e) => {
                                    eprintln!("error: {e}");
                                    process::exit(1);
                                }
                            }
                        }
                        SafariAiAction::Read { url } => {
                            match cueward_adapter_macos::safari::grok_read_conversation(&url, p) {
                                Ok(r) => {
                                    print_external(
                                        "safari/ai/grok/read",
                                        &serde_json::to_string_pretty(&r).unwrap(),
                                    );
                                    eprintln!("conversation read");
                                }
                                Err(e) => {
                                    eprintln!("error: {e}");
                                    process::exit(1);
                                }
                            }
                        }
                        _ => {
                            eprintln!("error: Grok currently supports only prompt, list, and read");
                            process::exit(1);
                        }
                    },
                    SafariAiProvider::Threads => match action {
                        SafariAiAction::List => {
                            match cueward_adapter_macos::safari::threads_extract_feed(p) {
                                Ok(posts) => {
                                    print_external(
                                        "safari/threads/feed",
                                        &serde_json::to_string_pretty(&posts).unwrap(),
                                    );
                                    eprintln!("{} post(s)", posts.len());
                                }
                                Err(e) => {
                                    eprintln!("error: {e}");
                                    process::exit(1);
                                }
                            }
                        }
                        _ => {
                            eprintln!("error: Threads currently supports only list");
                            process::exit(1);
                        }
                    },
                    SafariAiProvider::X => match action {
                        SafariAiAction::Prompt {
                            prompt,
                            mode,
                            auto_confirm,
                        } => {
                            if auto_confirm {
                                eprintln!("error: X prompt does not support --auto-confirm");
                                process::exit(1);
                            }
                            if mode.is_some() {
                                eprintln!("error: X prompt does not support --mode");
                                process::exit(1);
                            }
                            match cueward_adapter_macos::safari::x_search(&prompt, p) {
                                Ok(posts) => {
                                    print_external(
                                        "safari/x/search",
                                        &serde_json::to_string_pretty(&posts).unwrap(),
                                    );
                                    eprintln!("{} post(s)", posts.len());
                                }
                                Err(e) => {
                                    eprintln!("error: {e}");
                                    process::exit(1);
                                }
                            }
                        }
                        SafariAiAction::List => {
                            match cueward_adapter_macos::safari::x_extract_feed(p) {
                                Ok(posts) => {
                                    print_external(
                                        "safari/x/feed",
                                        &serde_json::to_string_pretty(&posts).unwrap(),
                                    );
                                    eprintln!("{} post(s)", posts.len());
                                }
                                Err(e) => {
                                    eprintln!("error: {e}");
                                    process::exit(1);
                                }
                            }
                        }
                        SafariAiAction::Read { url } => {
                            match cueward_adapter_macos::safari::x_read_post(&url, p) {
                                Ok(posts) => {
                                    print_external(
                                        "safari/x/read",
                                        &serde_json::to_string_pretty(&posts).unwrap(),
                                    );
                                    eprintln!("{} post(s)", posts.len());
                                }
                                Err(e) => {
                                    eprintln!("error: {e}");
                                    process::exit(1);
                                }
                            }
                        }
                        _ => {
                            eprintln!("error: X currently supports prompt, list, and read");
                            process::exit(1);
                        }
                    },
                }
            }
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
                    print_external(
                        "screenshot",
                        &serde_json::to_string_pretty(&result).unwrap(),
                    );
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
                        print_external(
                            "clipboard/get",
                            &serde_json::to_string_pretty(&content).unwrap(),
                        );
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
        Cli, Command, GeminiAiAction, GeminiMode, SafariAction, SafariAiAction, SafariAiProvider,
        SafariBookmarksAction, bookmarks_target_folder, build_gemini_ai_action, local_day_bounds,
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
    fn cli_parses_safari_exec_with_profile() {
        let cli = Cli::try_parse_from(["cueward", "safari", "exec", "--profile", "Ryugu", "1+1"])
            .expect("parse safari exec with profile");

        match cli.command {
            Command::Safari {
                action:
                    SafariAction::Exec {
                        js_code, profile, ..
                    },
            } => {
                assert_eq!(js_code, "1+1");
                assert_eq!(profile.as_deref(), Some("Ryugu"));
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn cli_parses_safari_active_with_profile() {
        let cli = Cli::try_parse_from(["cueward", "safari", "active", "--profile", "Ryugu"])
            .expect("parse safari active with profile");

        match cli.command {
            Command::Safari {
                action: SafariAction::Active { profile },
            } => assert_eq!(profile.as_deref(), Some("Ryugu")),
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn cli_parses_scroll_and_read() {
        let cli = Cli::try_parse_from([
            "cueward",
            "safari",
            "scroll-and-read",
            "--tab",
            "x.com",
            "--profile",
            "Ryugu",
            "--times",
            "3",
        ])
        .expect("parse scroll-and-read");

        match cli.command {
            Command::Safari {
                action:
                    SafariAction::ScrollAndRead {
                        tab,
                        profile,
                        times,
                        amount,
                        selector,
                    },
            } => {
                assert_eq!(tab.as_deref(), Some("x.com"));
                assert_eq!(profile.as_deref(), Some("Ryugu"));
                assert_eq!(times, 3);
                assert_eq!(amount, None);
                assert_eq!(selector, None);
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn cli_parses_safari_bookmarks_list_with_folder() {
        let cli = Cli::try_parse_from([
            "cueward",
            "safari",
            "bookmarks",
            "list",
            "--folder",
            "Work/AI Tools",
        ])
        .expect("parse safari bookmarks list");

        match cli.command {
            Command::Safari {
                action:
                    SafariAction::Bookmarks {
                        action: SafariBookmarksAction::List { profile, folder },
                    },
            } => {
                assert_eq!(profile, None);
                assert_eq!(folder.as_deref(), Some("Work/AI Tools"));
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn cli_parses_safari_bookmarks_list_with_profile() {
        let cli = Cli::try_parse_from([
            "cueward",
            "safari",
            "bookmarks",
            "list",
            "--profile",
            "Ryugu",
        ])
        .expect("parse safari bookmarks list with profile");

        match cli.command {
            Command::Safari {
                action:
                    SafariAction::Bookmarks {
                        action: SafariBookmarksAction::List { profile, folder },
                    },
            } => {
                assert_eq!(profile.as_deref(), Some("Ryugu"));
                assert_eq!(folder, None);
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn cli_parses_safari_bookmarks_search_with_folder() {
        let cli = Cli::try_parse_from([
            "cueward",
            "safari",
            "bookmarks",
            "search",
            "claude",
            "--folder",
            "Work/AI Tools",
        ])
        .expect("parse safari bookmarks search");

        match cli.command {
            Command::Safari {
                action:
                    SafariAction::Bookmarks {
                        action:
                            SafariBookmarksAction::Search {
                                query,
                                profile,
                                folder,
                            },
                    },
            } => {
                assert_eq!(query, "claude");
                assert_eq!(profile, None);
                assert_eq!(folder.as_deref(), Some("Work/AI Tools"));
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn cli_parses_safari_bookmarks_add_with_title_url_and_folder() {
        let cli = Cli::try_parse_from([
            "cueward",
            "safari",
            "bookmarks",
            "add",
            "--title",
            "Claude",
            "--url",
            "https://claude.ai",
            "--folder",
            "Work/AI Tools",
        ])
        .expect("parse safari bookmarks add");

        match cli.command {
            Command::Safari {
                action:
                    SafariAction::Bookmarks {
                        action:
                            SafariBookmarksAction::Add {
                                title,
                                url,
                                profile,
                                folder,
                            },
                    },
            } => {
                assert_eq!(title, "Claude");
                assert_eq!(url, "https://claude.ai");
                assert_eq!(profile, None);
                assert_eq!(folder.as_deref(), Some("Work/AI Tools"));
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn cli_parses_safari_bookmarks_add_with_profile_and_folder() {
        let cli = Cli::try_parse_from([
            "cueward",
            "safari",
            "bookmarks",
            "add",
            "--title",
            "Claude",
            "--url",
            "https://claude.ai",
            "--profile",
            "Ryugu",
            "--folder",
            "Work/AI Tools",
        ])
        .expect("parse safari bookmarks add with profile");

        match cli.command {
            Command::Safari {
                action:
                    SafariAction::Bookmarks {
                        action:
                            SafariBookmarksAction::Add {
                                title,
                                url,
                                profile,
                                folder,
                            },
                    },
            } => {
                assert_eq!(title, "Claude");
                assert_eq!(url, "https://claude.ai");
                assert_eq!(profile.as_deref(), Some("Ryugu"));
                assert_eq!(folder.as_deref(), Some("Work/AI Tools"));
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn bookmarks_target_folder_prepends_profile_to_folder() {
        let folder = bookmarks_target_folder(Some("Ryugu"), Some("Work/AI Tools"));

        assert_eq!(folder, Some("Ryugu/Work/AI Tools".to_string()));
    }

    #[test]
    fn bookmarks_target_folder_uses_profile_as_root_when_folder_missing() {
        let folder = bookmarks_target_folder(Some("Ryugu"), None);

        assert_eq!(folder, Some("Ryugu".to_string()));
    }

    #[test]
    fn cli_parses_safari_bookmarks_delete_with_title_url_and_folder() {
        let cli = Cli::try_parse_from([
            "cueward",
            "safari",
            "bookmarks",
            "delete",
            "--title",
            "Claude",
            "--url",
            "https://claude.ai",
            "--folder",
            "Work/AI Tools",
        ])
        .expect("parse safari bookmarks delete");

        match cli.command {
            Command::Safari {
                action:
                    SafariAction::Bookmarks {
                        action:
                            SafariBookmarksAction::Delete {
                                title,
                                url,
                                profile,
                                folder,
                            },
                    },
            } => {
                assert_eq!(title, "Claude");
                assert_eq!(url, "https://claude.ai");
                assert_eq!(profile, None);
                assert_eq!(folder.as_deref(), Some("Work/AI Tools"));
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn cli_parses_gemini_prompt_with_mode() {
        let cli = Cli::try_parse_from([
            "cueward",
            "safari",
            "ai",
            "--provider",
            "gemini",
            "prompt",
            "--prompt",
            "研究議題",
            "--mode",
            "deep-research",
        ])
        .expect("parse");

        match cli.command {
            Command::Safari {
                action:
                    SafariAction::Ai {
                        provider,
                        action:
                            SafariAiAction::Prompt {
                                prompt,
                                mode,
                                auto_confirm,
                            },
                        ..
                    },
            } => {
                assert_eq!(provider, SafariAiProvider::Gemini);
                assert_eq!(prompt, "研究議題");
                assert_eq!(mode, Some(GeminiMode::DeepResearch));
                assert!(!auto_confirm);
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn cli_parses_gemini_prompt_only() {
        let cli = Cli::try_parse_from([
            "cueward",
            "safari",
            "ai",
            "--provider",
            "gemini",
            "prompt",
            "--prompt",
            "哈囉",
        ])
        .expect("parse");

        match cli.command {
            Command::Safari {
                action:
                    SafariAction::Ai {
                        action: SafariAiAction::Prompt { prompt, mode, .. },
                        ..
                    },
            } => {
                assert_eq!(prompt, "哈囉");
                assert_eq!(mode, None);
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn cli_parses_chatgpt_prompt_only() {
        let cli = Cli::try_parse_from([
            "cueward",
            "safari",
            "ai",
            "--provider",
            "chatgpt",
            "prompt",
            "--prompt",
            "哈囉 ChatGPT",
        ])
        .expect("parse");

        match cli.command {
            Command::Safari {
                action:
                    SafariAction::Ai {
                        provider,
                        action:
                            SafariAiAction::Prompt {
                                prompt,
                                mode,
                                auto_confirm,
                            },
                        ..
                    },
            } => {
                assert_eq!(provider, SafariAiProvider::Chatgpt);
                assert_eq!(prompt, "哈囉 ChatGPT");
                assert_eq!(mode, None);
                assert!(!auto_confirm);
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn cli_parses_chatgpt_prompt_with_image_mode() {
        let cli = Cli::try_parse_from([
            "cueward",
            "safari",
            "ai",
            "--provider",
            "chatgpt",
            "prompt",
            "--prompt",
            "畫一隻貓",
            "--mode",
            "image",
        ])
        .expect("parse");

        match cli.command {
            Command::Safari {
                action:
                    SafariAction::Ai {
                        provider,
                        action:
                            SafariAiAction::Prompt {
                                prompt,
                                mode,
                                auto_confirm,
                            },
                        ..
                    },
            } => {
                assert_eq!(provider, SafariAiProvider::Chatgpt);
                assert_eq!(prompt, "畫一隻貓");
                assert_eq!(mode, Some(GeminiMode::Image));
                assert!(!auto_confirm);
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn cli_parses_gemini_auto_confirm() {
        let cli = Cli::try_parse_from([
            "cueward",
            "safari",
            "ai",
            "--provider",
            "gemini",
            "prompt",
            "--prompt",
            "研究主題",
            "--mode",
            "deep-research",
            "--auto-confirm",
        ])
        .expect("parse");

        match cli.command {
            Command::Safari {
                action:
                    SafariAction::Ai {
                        action: SafariAiAction::Prompt { auto_confirm, .. },
                        ..
                    },
            } => {
                assert!(auto_confirm);
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn cli_parses_gemini_list() {
        let cli = Cli::try_parse_from(["cueward", "safari", "ai", "--provider", "gemini", "list"])
            .expect("parse");

        match cli.command {
            Command::Safari {
                action:
                    SafariAction::Ai {
                        provider,
                        action: SafariAiAction::List,
                        ..
                    },
            } => {
                assert_eq!(provider, SafariAiProvider::Gemini);
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn cli_parses_grok_prompt_only() {
        let cli = Cli::try_parse_from([
            "cueward",
            "safari",
            "ai",
            "--provider",
            "grok",
            "prompt",
            "--prompt",
            "哈囉 Grok",
        ])
        .expect("parse");

        match cli.command {
            Command::Safari {
                action:
                    SafariAction::Ai {
                        provider,
                        action:
                            SafariAiAction::Prompt {
                                prompt,
                                mode,
                                auto_confirm,
                            },
                        ..
                    },
            } => {
                assert_eq!(provider, SafariAiProvider::Grok);
                assert_eq!(prompt, "哈囉 Grok");
                assert_eq!(mode, None);
                assert!(!auto_confirm);
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn cli_parses_grok_list() {
        let cli = Cli::try_parse_from(["cueward", "safari", "ai", "--provider", "grok", "list"])
            .expect("parse");

        match cli.command {
            Command::Safari {
                action:
                    SafariAction::Ai {
                        provider,
                        action: SafariAiAction::List,
                        ..
                    },
            } => {
                assert_eq!(provider, SafariAiProvider::Grok);
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn cli_parses_grok_read() {
        let cli = Cli::try_parse_from([
            "cueward",
            "safari",
            "ai",
            "--provider",
            "grok",
            "read",
            "https://grok.com/c/abc",
        ])
        .expect("parse");

        match cli.command {
            Command::Safari {
                action:
                    SafariAction::Ai {
                        provider,
                        action: SafariAiAction::Read { url },
                        ..
                    },
            } => {
                assert_eq!(provider, SafariAiProvider::Grok);
                assert_eq!(url, "https://grok.com/c/abc");
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn cli_parses_chatgpt_list() {
        let cli = Cli::try_parse_from(["cueward", "safari", "ai", "--provider", "chatgpt", "list"])
            .expect("parse");

        match cli.command {
            Command::Safari {
                action:
                    SafariAction::Ai {
                        provider,
                        action: SafariAiAction::List,
                        ..
                    },
            } => {
                assert_eq!(provider, SafariAiProvider::Chatgpt);
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn cli_parses_x_prompt() {
        let cli = Cli::try_parse_from([
            "cueward",
            "safari",
            "ai",
            "--provider",
            "x",
            "prompt",
            "--prompt",
            "Claude Code",
        ])
        .expect("parse");

        match cli.command {
            Command::Safari {
                action:
                    SafariAction::Ai {
                        provider,
                        action: SafariAiAction::Prompt { prompt, .. },
                        ..
                    },
            } => {
                assert_eq!(provider, SafariAiProvider::X);
                assert_eq!(prompt, "Claude Code");
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn cli_parses_x_read() {
        let cli = Cli::try_parse_from([
            "cueward",
            "safari",
            "ai",
            "--provider",
            "x",
            "read",
            "https://x.com/openai/status/123",
        ])
        .expect("parse");

        match cli.command {
            Command::Safari {
                action:
                    SafariAction::Ai {
                        provider,
                        action: SafariAiAction::Read { url },
                        ..
                    },
            } => {
                assert_eq!(provider, SafariAiProvider::X);
                assert_eq!(url, "https://x.com/openai/status/123");
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn cli_parses_gemini_poll() {
        let cli = Cli::try_parse_from([
            "cueward",
            "safari",
            "ai",
            "--provider",
            "gemini",
            "poll",
            "--timeout",
            "120",
        ])
        .expect("parse");

        match cli.command {
            Command::Safari {
                action:
                    SafariAction::Ai {
                        action: SafariAiAction::Poll { timeout },
                        ..
                    },
            } => {
                assert_eq!(timeout, 120);
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn cli_parses_gemini_with_profile() {
        let cli = Cli::try_parse_from([
            "cueward",
            "safari",
            "ai",
            "--provider",
            "gemini",
            "--profile",
            "Ryugu",
            "prompt",
            "--prompt",
            "hi",
        ])
        .expect("parse");

        match cli.command {
            Command::Safari {
                action: SafariAction::Ai { profile, .. },
            } => {
                assert_eq!(profile.as_deref(), Some("Ryugu"));
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn build_gemini_ai_action_allows_mode_and_prompt_together() {
        assert_eq!(
            build_gemini_ai_action(Some(GeminiMode::DeepResearch), Some("研究主題"), false)
                .unwrap(),
            GeminiAiAction::DeepResearchPlan("研究主題".to_string(), false)
        );
    }

    #[test]
    fn build_gemini_ai_action_rejects_invalid_auto_confirm_usage() {
        assert_eq!(
            build_gemini_ai_action(Some(GeminiMode::Image), None, true),
            Err("--auto-confirm requires --mode deep-research and --prompt")
        );
    }
}
