# Calendar / Screenshot / Clipboard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add three macOS integrations to cueward: calendar read/write, screenshot with optional OCR, clipboard read/write.

**Architecture:** Each feature is a new module in `crates/adapter-macos/src/` with a corresponding CLI subcommand in `crates/cli/src/main.rs`. Calendar and clipboard use AppleScript via the existing `applescript::run` helper (extended with `run_capture` for stdout). Screenshot uses macOS `screencapture` command. All follow the existing pattern: adapter module → CLI handler → JSON stdout / stderr status.

**Tech Stack:** Rust, clap (CLI), AppleScript via osascript, macOS screencapture, existing Vision OCR pipeline

---

## File Structure

| Action | Path | Responsibility |
|--------|------|----------------|
| Modify | `crates/adapter-macos/src/applescript.rs` | Add `run_capture()` that returns stdout |
| Create | `crates/adapter-macos/src/calendar.rs` | Calendar CRUD via AppleScript |
| Create | `crates/adapter-macos/src/screenshot.rs` | screencapture + optional OCR |
| Create | `crates/adapter-macos/src/clipboard.rs` | pbpaste/pbcopy + image detection |
| Modify | `crates/adapter-macos/src/lib.rs` | Export new modules |
| Modify | `crates/cli/src/main.rs` | Add Calendar, Screenshot, Clipboard subcommands |

---

### Task 1: Extend applescript helper with `run_capture`

Calendar list needs to capture AppleScript stdout. Currently `applescript::run` discards it.

**Files:**
- Modify: `crates/adapter-macos/src/applescript.rs`

- [ ] **Step 1: Add `run_capture` function**

Add after the existing `run` function in `crates/adapter-macos/src/applescript.rs`:

```rust
/// Run an AppleScript and return its stdout on success.
pub fn run_capture(script: &str, context: &str) -> Result<String, MacosError> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|e| MacosError::Other(format!("osascript: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MacosError::Other(format!("{context}: {stderr}")));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd /Users/cyh/Development/cueward && cargo check -p cueward-adapter-macos 2>&1 | tail -5`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
cd /Users/cyh/Development/cueward
git add crates/adapter-macos/src/applescript.rs
git commit -m "feat(applescript): add run_capture to return stdout"
```

---

### Task 2: Calendar module

**Files:**
- Create: `crates/adapter-macos/src/calendar.rs`
- Modify: `crates/adapter-macos/src/lib.rs`

- [ ] **Step 1: Create calendar.rs with CalendarEvent struct and list_events**

Create `crates/adapter-macos/src/calendar.rs`:

```rust
use std::process::Command;

use chrono::{DateTime, Local, NaiveDateTime, Utc};
use serde::Serialize;

use crate::applescript::escape;
use crate::MacosError;

#[derive(Debug, Serialize)]
pub struct CalendarEvent {
    pub title: String,
    pub start: String,
    pub end: String,
    pub calendar: String,
    pub location: String,
    pub notes: String,
    pub all_day: bool,
}

/// Format a DateTime for AppleScript: "YYYY-MM-DD HH:MM:SS"
fn format_for_applescript(dt: &DateTime<Local>) -> String {
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Parse a single tab-separated line from the AppleScript output into a CalendarEvent.
fn parse_event_line(line: &str) -> Option<CalendarEvent> {
    // Fields: title \t start \t end \t calendar \t location \t notes \t all_day
    let parts: Vec<&str> = line.split('\t').collect();
    if parts.len() < 7 {
        return None;
    }
    Some(CalendarEvent {
        title: parts[0].to_string(),
        start: parts[1].to_string(),
        end: parts[2].to_string(),
        calendar: parts[3].to_string(),
        location: parts[4].to_string(),
        notes: parts[5].to_string(),
        all_day: parts[6].trim() == "true",
    })
}

/// List calendar events between two dates.
pub fn list_events(
    from: DateTime<Local>,
    to: DateTime<Local>,
    calendar_filter: Option<&str>,
) -> Result<Vec<CalendarEvent>, MacosError> {
    let from_str = format_for_applescript(&from);
    let to_str = format_for_applescript(&to);

    // AppleScript to query events. Output tab-separated fields, one event per line.
    let calendar_clause = match calendar_filter {
        Some(name) => format!(r#"set cals to {{calendar "{}"}}"#, escape(name)),
        None => "set cals to every calendar".to_string(),
    };

    let script = format!(
        r#"
        tell application "Calendar"
            {calendar_clause}
            set output to ""
            set fromDate to date "{from_str}"
            set toDate to date "{to_str}"
            repeat with cal in cals
                set calName to name of cal
                set evts to (every event of cal whose start date ≥ fromDate and start date ≤ toDate)
                repeat with evt in evts
                    set evtTitle to summary of evt
                    set evtStart to start date of evt
                    set evtEnd to end date of evt
                    set evtLoc to ""
                    try
                        set evtLoc to location of evt
                    end try
                    if evtLoc is missing value then set evtLoc to ""
                    set evtNotes to ""
                    try
                        set evtNotes to description of evt
                    end try
                    if evtNotes is missing value then set evtNotes to ""
                    set isAllDay to allday event of evt
                    set evtStartStr to (evtStart as «class isot» as string)
                    set evtEndStr to (evtEnd as «class isot» as string)
                    set output to output & evtTitle & tab & evtStartStr & tab & evtEndStr & tab & calName & tab & evtLoc & tab & evtNotes & tab & isAllDay & linefeed
                end repeat
            end repeat
            return output
        end tell
        "#
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| MacosError::Other(format!("osascript: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MacosError::Other(format!("failed to list events: {stderr}")));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let events: Vec<CalendarEvent> = stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(parse_event_line)
        .collect();

    Ok(events)
}

/// Create a calendar event.
pub fn create_event(
    title: &str,
    start: &DateTime<Local>,
    end: &DateTime<Local>,
    calendar_name: Option<&str>,
    notes: Option<&str>,
    location: Option<&str>,
) -> Result<(), MacosError> {
    let escaped_title = escape(title);
    let start_str = format_for_applescript(start);
    let end_str = format_for_applescript(end);

    let cal_clause = match calendar_name {
        Some(name) => format!(r#"set targetCal to calendar "{}""#, escape(name)),
        None => "set targetCal to default calendar".to_string(),
    };

    let mut props = format!(
        r#"summary:"{escaped_title}", start date:date "{start_str}", end date:date "{end_str}""#
    );
    if let Some(loc) = location {
        props.push_str(&format!(r#", location:"{}""#, escape(loc)));
    }
    if let Some(n) = notes {
        props.push_str(&format!(r#", description:"{}""#, escape(n)));
    }

    let script = format!(
        r#"
        tell application "Calendar"
            {cal_clause}
            make new event at end of events of targetCal with properties {{{props}}}
        end tell
        "#
    );

    crate::applescript::run(&script, "failed to create event")
}

/// Delete a calendar event by title and start date.
pub fn delete_event(
    title: &str,
    start: &DateTime<Local>,
    calendar_name: Option<&str>,
) -> Result<(), MacosError> {
    let escaped_title = escape(title);
    let start_str = format_for_applescript(start);

    let cal_clause = match calendar_name {
        Some(name) => format!(r#"set cals to {{calendar "{}"}}"#, escape(name)),
        None => "set cals to every calendar".to_string(),
    };

    let script = format!(
        r#"
        tell application "Calendar"
            {cal_clause}
            set targetDate to date "{start_str}"
            set found to false
            repeat with cal in cals
                set evts to (every event of cal whose summary is "{escaped_title}" and start date is targetDate)
                repeat with evt in evts
                    delete evt
                    set found to true
                end repeat
            end repeat
            if not found then error "event not found: {escaped_title} at {start_str}"
        end tell
        "#
    );

    crate::applescript::run(&script, "failed to delete event")
}
```

- [ ] **Step 2: Export calendar module from lib.rs**

Add to `crates/adapter-macos/src/lib.rs` after `pub mod quick_notes;`:

```rust
pub mod calendar;
```

- [ ] **Step 3: Verify it compiles**

Run: `cd /Users/cyh/Development/cueward && cargo check -p cueward-adapter-macos 2>&1 | tail -5`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
cd /Users/cyh/Development/cueward
git add crates/adapter-macos/src/calendar.rs crates/adapter-macos/src/lib.rs
git commit -m "feat(calendar): add list/create/delete via AppleScript"
```

---

### Task 3: Calendar CLI subcommand

**Files:**
- Modify: `crates/cli/src/main.rs`

- [ ] **Step 1: Add CalendarAction enum and Calendar command variant**

Add after the `QuickNotesAction` enum (around line 183):

```rust
#[derive(Subcommand)]
enum CalendarAction {
    /// List events in a time range
    List {
        /// Start of range (ISO 8601 or "YYYY-MM-DD HH:MM")
        #[arg(long)]
        from: Option<String>,

        /// End of range (ISO 8601 or "YYYY-MM-DD HH:MM")
        #[arg(long)]
        to: Option<String>,

        /// Filter by calendar name
        #[arg(long)]
        calendar: Option<String>,
    },

    /// List today's events (shortcut for list --from "today 00:00" --to "today 23:59")
    Today,

    /// Create a new calendar event
    Create {
        /// Event title
        #[arg(long)]
        title: String,

        /// Start time (ISO 8601 or "YYYY-MM-DD HH:MM")
        #[arg(long)]
        start: String,

        /// End time (ISO 8601 or "YYYY-MM-DD HH:MM")
        #[arg(long)]
        end: String,

        /// Calendar name (uses default if not specified)
        #[arg(long)]
        calendar: Option<String>,

        /// Event notes/description
        #[arg(long)]
        notes: Option<String>,

        /// Event location
        #[arg(long)]
        location: Option<String>,
    },

    /// Delete a calendar event
    Delete {
        /// Event title
        #[arg(long)]
        title: String,

        /// Event start time (to identify the specific occurrence)
        #[arg(long)]
        start: String,

        /// Calendar name (searches all if not specified)
        #[arg(long)]
        calendar: Option<String>,
    },
}
```

Add the `Calendar` variant to the `Command` enum:

```rust
    /// Manage Apple Calendar events
    Calendar {
        #[command(subcommand)]
        action: CalendarAction,
    },
```

- [ ] **Step 2: Add datetime parser helper**

Add after `parse_duration` function:

```rust
fn parse_datetime(s: &str) -> Option<DateTime<Local>> {
    use chrono::{Local, NaiveDateTime, TimeZone};
    // Try ISO 8601 with timezone
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Local));
    }
    // Try "YYYY-MM-DD HH:MM:SS"
    if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Local.from_local_datetime(&ndt).single();
    }
    // Try "YYYY-MM-DD HH:MM"
    if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M") {
        return Local.from_local_datetime(&ndt).single();
    }
    None
}
```

Add to imports at top of main.rs:

```rust
use chrono::{DateTime, Local, TimeZone, Utc};
```

- [ ] **Step 3: Add Calendar command handler**

Add in the main `match cli.command` block, before the closing `}`:

```rust
        Command::Calendar { action } => match action {
            CalendarAction::List { from, to, calendar } => {
                let from_dt = match from {
                    Some(s) => match parse_datetime(&s) {
                        Some(dt) => dt,
                        None => {
                            eprintln!("error: invalid --from datetime '{s}' (use YYYY-MM-DD HH:MM)");
                            process::exit(1);
                        }
                    },
                    None => Local::now(),
                };
                let to_dt = match to {
                    Some(s) => match parse_datetime(&s) {
                        Some(dt) => dt,
                        None => {
                            eprintln!("error: invalid --to datetime '{s}' (use YYYY-MM-DD HH:MM)");
                            process::exit(1);
                        }
                    },
                    None => from_dt + chrono::Duration::hours(24),
                };
                match cueward_adapter_macos::calendar::list_events(from_dt, to_dt, calendar.as_deref()) {
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
            CalendarAction::Today => {
                let today_start = Local::now()
                    .date_naive()
                    .and_hms_opt(0, 0, 0)
                    .and_then(|ndt| Local.from_local_datetime(&ndt).single())
                    .unwrap();
                let today_end = Local::now()
                    .date_naive()
                    .and_hms_opt(23, 59, 59)
                    .and_then(|ndt| Local.from_local_datetime(&ndt).single())
                    .unwrap();
                match cueward_adapter_macos::calendar::list_events(today_start, today_end, None) {
                    Ok(events) => {
                        println!("{}", serde_json::to_string_pretty(&events).unwrap());
                        eprintln!("{} event(s) today", events.len());
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            }
            CalendarAction::Create { title, start, end, calendar, notes, location } => {
                let start_dt = match parse_datetime(&start) {
                    Some(dt) => dt,
                    None => {
                        eprintln!("error: invalid --start datetime '{start}'");
                        process::exit(1);
                    }
                };
                let end_dt = match parse_datetime(&end) {
                    Some(dt) => dt,
                    None => {
                        eprintln!("error: invalid --end datetime '{end}'");
                        process::exit(1);
                    }
                };
                match cueward_adapter_macos::calendar::create_event(
                    &title, &start_dt, &end_dt, calendar.as_deref(), notes.as_deref(), location.as_deref(),
                ) {
                    Ok(()) => eprintln!("event created: {title}"),
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            }
            CalendarAction::Delete { title, start, calendar } => {
                let start_dt = match parse_datetime(&start) {
                    Some(dt) => dt,
                    None => {
                        eprintln!("error: invalid --start datetime '{start}'");
                        process::exit(1);
                    }
                };
                match cueward_adapter_macos::calendar::delete_event(
                    &title, &start_dt, calendar.as_deref(),
                ) {
                    Ok(()) => eprintln!("event deleted: {title}"),
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            }
        },
```

- [ ] **Step 4: Verify it compiles**

Run: `cd /Users/cyh/Development/cueward && cargo check 2>&1 | tail -5`
Expected: no errors

- [ ] **Step 5: Test manually**

Run: `cd /Users/cyh/Development/cueward && cargo run -- calendar today 2>&1`
Expected: JSON output of today's events (or empty array)

- [ ] **Step 6: Commit**

```bash
cd /Users/cyh/Development/cueward
git add crates/cli/src/main.rs
git commit -m "feat(cli): add calendar subcommand (list/today/create/delete)"
```

---

### Task 4: Screenshot module

**Files:**
- Create: `crates/adapter-macos/src/screenshot.rs`
- Modify: `crates/adapter-macos/src/lib.rs`

- [ ] **Step 1: Create screenshot.rs**

Create `crates/adapter-macos/src/screenshot.rs`:

```rust
use std::fs;
use std::process::Command;

use chrono::Local;
use serde::Serialize;

use crate::MacosError;

#[derive(Debug, Serialize)]
pub struct ScreenshotResult {
    pub path: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ocr_text: Option<String>,
}

const CACHE_DIR: &str = ".cueward/cache/screenshots";

fn ensure_cache_dir() -> Result<String, MacosError> {
    let home = std::env::var("HOME")
        .map_err(|_| MacosError::Other("HOME not set".into()))?;
    let dir = format!("{home}/{CACHE_DIR}");
    fs::create_dir_all(&dir)
        .map_err(|e| MacosError::Other(format!("failed to create {dir}: {e}")))?;
    Ok(dir)
}

/// Capture a screenshot of the entire screen.
/// If `ocr` is true, also runs Vision OCR on the captured image.
/// If `output` is Some, saves to that path instead of the default cache dir.
pub fn capture(ocr: bool, output: Option<&str>) -> Result<ScreenshotResult, MacosError> {
    let now = Local::now();
    let timestamp = now.format("%Y%m%d-%H%M%S").to_string();

    let path = match output {
        Some(p) => p.to_string(),
        None => {
            let dir = ensure_cache_dir()?;
            format!("{dir}/{timestamp}.png")
        }
    };

    // -x = silent (no shutter sound)
    let status = Command::new("screencapture")
        .args(["-x", &path])
        .status()
        .map_err(|e| MacosError::Other(format!("screencapture: {e}")))?;

    if !status.success() {
        return Err(MacosError::Other("screencapture failed".into()));
    }

    let ocr_text = if ocr {
        match crate::ocr::capture(&path) {
            Ok(cues) => {
                let text: String = cues.into_iter().map(|c| c.content).collect::<Vec<_>>().join("\n");
                if text.is_empty() { None } else { Some(text) }
            }
            Err(e) => {
                eprintln!("warning: OCR failed: {e}");
                None
            }
        }
    } else {
        None
    };

    Ok(ScreenshotResult {
        path,
        timestamp: now.to_rfc3339(),
        ocr_text,
    })
}
```

- [ ] **Step 2: Export screenshot module from lib.rs**

Add to `crates/adapter-macos/src/lib.rs`:

```rust
pub mod screenshot;
```

- [ ] **Step 3: Fix ocr module visibility**

The `screenshot` module needs to call `crate::ocr::capture`, but `ocr` is currently `pub mod ocr`. Verify it's accessible — it already is (`pub mod ocr` in lib.rs). No change needed.

- [ ] **Step 4: Verify it compiles**

Run: `cd /Users/cyh/Development/cueward && cargo check -p cueward-adapter-macos 2>&1 | tail -5`
Expected: no errors

- [ ] **Step 5: Commit**

```bash
cd /Users/cyh/Development/cueward
git add crates/adapter-macos/src/screenshot.rs crates/adapter-macos/src/lib.rs
git commit -m "feat(screenshot): screencapture with optional OCR"
```

---

### Task 5: Screenshot CLI subcommand

**Files:**
- Modify: `crates/cli/src/main.rs`

- [ ] **Step 1: Add Screenshot command variant**

Add to the `Command` enum:

```rust
    /// Capture a screenshot of the entire screen
    Screenshot {
        /// Also run OCR on the captured image
        #[arg(long)]
        ocr: bool,

        /// Output path (default: ~/.cueward/cache/screenshots/<timestamp>.png)
        #[arg(long)]
        output: Option<String>,
    },
```

- [ ] **Step 2: Add Screenshot handler**

Add in the main `match cli.command` block:

```rust
        Command::Screenshot { ocr, output } => {
            match cueward_adapter_macos::screenshot::capture(ocr, output.as_deref()) {
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
```

- [ ] **Step 3: Verify it compiles and test**

Run: `cd /Users/cyh/Development/cueward && cargo run -- screenshot 2>&1`
Expected: JSON with path to screenshot file

Run: `cd /Users/cyh/Development/cueward && cargo run -- screenshot --ocr 2>&1`
Expected: JSON with path + ocr_text field

- [ ] **Step 4: Commit**

```bash
cd /Users/cyh/Development/cueward
git add crates/cli/src/main.rs
git commit -m "feat(cli): add screenshot subcommand"
```

---

### Task 6: Clipboard module

**Files:**
- Create: `crates/adapter-macos/src/clipboard.rs`
- Modify: `crates/adapter-macos/src/lib.rs`

- [ ] **Step 1: Create clipboard.rs**

Create `crates/adapter-macos/src/clipboard.rs`:

```rust
use std::fs;
use std::process::Command;

use chrono::Local;
use serde::Serialize;

use crate::MacosError;

#[derive(Debug, Serialize)]
pub struct ClipboardContent {
    #[serde(rename = "type")]
    pub content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

const CACHE_DIR: &str = ".cueward/cache/clipboard";

fn ensure_cache_dir() -> Result<String, MacosError> {
    let home = std::env::var("HOME")
        .map_err(|_| MacosError::Other("HOME not set".into()))?;
    let dir = format!("{home}/{CACHE_DIR}");
    fs::create_dir_all(&dir)
        .map_err(|e| MacosError::Other(format!("failed to create {dir}: {e}")))?;
    Ok(dir)
}

/// Check if clipboard contains an image.
fn has_image() -> bool {
    let output = Command::new("osascript")
        .arg("-e")
        .arg("clipboard info")
        .output();
    match output {
        Ok(o) => {
            let info = String::from_utf8_lossy(&o.stdout);
            info.contains("«class PNGf»") || info.contains("«class TIFF»")
        }
        Err(_) => false,
    }
}

/// Save clipboard image to a PNG file using AppleScript + osascript.
fn save_clipboard_image(save_path: &str) -> Result<(), MacosError> {
    // Use osascript with a small script to write clipboard image as PNG
    let script = format!(
        r#"
        use framework "AppKit"
        set pb to current application's NSPasteboard's generalPasteboard()
        set imgData to pb's dataForType:(current application's NSPasteboardTypePNG)
        if imgData is missing value then
            set tiffData to pb's dataForType:(current application's NSPasteboardTypeTIFF)
            if tiffData is missing value then error "no image in clipboard"
            set bitmapRep to current application's NSBitmapImageRep's imageRepWithData:tiffData
            set imgData to bitmapRep's representationUsingType:(current application's NSBitmapImageFileTypePNG) properties:(missing value)
        end if
        imgData's writeToFile:"{save_path}" atomically:true
        "#,
        save_path = save_path.replace('"', "\\\""),
    );

    let output = Command::new("osascript")
        .arg("-l")
        .arg("AppleScript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| MacosError::Other(format!("osascript: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MacosError::Other(format!("failed to save clipboard image: {stderr}")));
    }

    Ok(())
}

/// Read clipboard content. Returns text or saves image and returns path.
pub fn get(save_image_path: Option<&str>) -> Result<ClipboardContent, MacosError> {
    // Check for image first
    if has_image() {
        let path = match save_image_path {
            Some(p) => p.to_string(),
            None => {
                let dir = ensure_cache_dir()?;
                let ts = Local::now().format("%Y%m%d-%H%M%S").to_string();
                format!("{dir}/{ts}.png")
            }
        };

        save_clipboard_image(&path)?;

        return Ok(ClipboardContent {
            content_type: "image".into(),
            content: None,
            path: Some(path),
        });
    }

    // Fall back to text
    let output = Command::new("pbpaste")
        .output()
        .map_err(|e| MacosError::Other(format!("pbpaste: {e}")))?;

    let text = String::from_utf8_lossy(&output.stdout).into_owned();

    Ok(ClipboardContent {
        content_type: "text".into(),
        content: Some(text),
        path: None,
    })
}

/// Write text to clipboard.
pub fn set(text: &str) -> Result<(), MacosError> {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = Command::new("pbcopy")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| MacosError::Other(format!("pbcopy: {e}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| MacosError::Other(format!("failed to write to pbcopy: {e}")))?;
    }

    let status = child
        .wait()
        .map_err(|e| MacosError::Other(format!("pbcopy: {e}")))?;

    if !status.success() {
        return Err(MacosError::Other("pbcopy failed".into()));
    }

    Ok(())
}
```

- [ ] **Step 2: Export clipboard module from lib.rs**

Add to `crates/adapter-macos/src/lib.rs`:

```rust
pub mod clipboard;
```

- [ ] **Step 3: Verify it compiles**

Run: `cd /Users/cyh/Development/cueward && cargo check -p cueward-adapter-macos 2>&1 | tail -5`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
cd /Users/cyh/Development/cueward
git add crates/adapter-macos/src/clipboard.rs crates/adapter-macos/src/lib.rs
git commit -m "feat(clipboard): get/set with text and image support"
```

---

### Task 7: Clipboard CLI subcommand

**Files:**
- Modify: `crates/cli/src/main.rs`

- [ ] **Step 1: Add ClipboardAction enum and Clipboard command variant**

Add `ClipboardAction` enum:

```rust
#[derive(Subcommand)]
enum ClipboardAction {
    /// Read clipboard content (text or image)
    Get {
        /// Save image to this path (default: ~/.cueward/cache/clipboard/<timestamp>.png)
        #[arg(long)]
        save_image: Option<String>,
    },

    /// Write text to clipboard
    Set {
        /// Text to copy to clipboard
        text: String,
    },
}
```

Add `Clipboard` to `Command` enum:

```rust
    /// Read or write the system clipboard
    Clipboard {
        #[command(subcommand)]
        action: ClipboardAction,
    },
```

- [ ] **Step 2: Add Clipboard handler**

```rust
        Command::Clipboard { action } => match action {
            ClipboardAction::Get { save_image } => {
                match cueward_adapter_macos::clipboard::get(save_image.as_deref()) {
                    Ok(content) => {
                        println!("{}", serde_json::to_string_pretty(&content).unwrap());
                        match content.content_type.as_str() {
                            "image" => eprintln!("clipboard image saved to {}", content.path.unwrap_or_default()),
                            _ => eprintln!("clipboard text read"),
                        }
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            }
            ClipboardAction::Set { text } => {
                match cueward_adapter_macos::clipboard::set(&text) {
                    Ok(()) => eprintln!("copied to clipboard"),
                    Err(e) => {
                        eprintln!("error: {e}");
                        process::exit(1);
                    }
                }
            }
        },
```

- [ ] **Step 3: Verify it compiles and test**

Run: `cd /Users/cyh/Development/cueward && cargo check 2>&1 | tail -5`
Expected: no errors

Test read: `cd /Users/cyh/Development/cueward && echo "hello" | pbcopy && cargo run -- clipboard get 2>&1`
Expected: JSON with `type: "text"`, `content: "hello\n"`

Test write: `cd /Users/cyh/Development/cueward && cargo run -- clipboard set "written by cueward" && pbpaste`
Expected: `written by cueward`

- [ ] **Step 4: Commit**

```bash
cd /Users/cyh/Development/cueward
git add crates/cli/src/main.rs
git commit -m "feat(cli): add clipboard subcommand (get/set)"
```

---

### Task 8: Build, install, and end-to-end test

**Files:**
- No new files

- [ ] **Step 1: Full build**

Run: `cd /Users/cyh/Development/cueward && cargo build --release 2>&1 | tail -5`
Expected: Compiling... Finished

- [ ] **Step 2: Install**

Run: `cd /Users/cyh/Development/cueward && cargo install --path crates/cli 2>&1 | tail -3`
Expected: Installing cueward... Installed

- [ ] **Step 3: Test calendar**

Run: `cueward calendar today 2>&1`
Expected: JSON array of today's events

- [ ] **Step 4: Test screenshot**

Run: `cueward screenshot --ocr 2>&1`
Expected: JSON with path and ocr_text

- [ ] **Step 5: Test clipboard roundtrip**

Run: `cueward clipboard set "cueward test" && cueward clipboard get 2>&1`
Expected: JSON with content "cueward test"

- [ ] **Step 6: Verify help output**

Run: `cueward --help 2>&1`
Expected: calendar, screenshot, clipboard all listed

- [ ] **Step 7: Final commit**

```bash
cd /Users/cyh/Development/cueward
git add -A
git commit -m "build: release build with calendar, screenshot, clipboard"
```

---

### Task 9: Update Ryugu daemon tools/cueward.ts

**Files:**
- Modify: `/Users/cyh/Development/Ryugu/daemon/src/tools/cueward.ts`
- Modify: `/Users/cyh/Development/Ryugu/daemon/src/tools/tools.test.ts`

- [ ] **Step 1: Add new actions to VALID_ACTIONS and subcommand whitelists**

In `cueward.ts`, update `VALID_ACTIONS`:

```typescript
const VALID_ACTIONS = [
  "capture", "search", "send", "plan", "ocr",
  "notes", "quick-notes", "triage",
  "calendar", "screenshot", "clipboard",
] as const;
```

Add subcommand whitelists:

```typescript
const VALID_CALENDAR_SUBS = ["list", "today", "create", "delete"] as const;
const VALID_CLIPBOARD_SUBS = ["get", "set"] as const;
```

- [ ] **Step 2: Add cases to buildCuewardCmd switch**

```typescript
    case "calendar": {
      const subcommand = String(args.subcommand ?? "");
      if (!VALID_CALENDAR_SUBS.includes(subcommand as any)) {
        throw new Error(`Invalid calendar subcommand: "${subcommand}". Valid: ${VALID_CALENDAR_SUBS.join(", ")}`);
      }
      let cmd = `${bin} calendar ${subcommand}`;
      if (args.from) cmd += ` --from ${escapeArg(String(args.from))}`;
      if (args.to) cmd += ` --to ${escapeArg(String(args.to))}`;
      if (args.calendar) cmd += ` --calendar ${escapeArg(String(args.calendar))}`;
      if (args.title) cmd += ` --title ${escapeArg(String(args.title))}`;
      if (args.start) cmd += ` --start ${escapeArg(String(args.start))}`;
      if (args.end) cmd += ` --end ${escapeArg(String(args.end))}`;
      if (args.notes) cmd += ` --notes ${escapeArg(String(args.notes))}`;
      if (args.location) cmd += ` --location ${escapeArg(String(args.location))}`;
      return cmd;
    }

    case "screenshot": {
      let cmd = `${bin} screenshot`;
      if (args.ocr) cmd += ` --ocr`;
      if (args.output) cmd += ` --output ${escapeArg(String(args.output))}`;
      return cmd;
    }

    case "clipboard": {
      const subcommand = String(args.subcommand ?? "");
      if (!VALID_CLIPBOARD_SUBS.includes(subcommand as any)) {
        throw new Error(`Invalid clipboard subcommand: "${subcommand}". Valid: ${VALID_CLIPBOARD_SUBS.join(", ")}`);
      }
      let cmd = `${bin} clipboard ${subcommand}`;
      if (subcommand === "get" && args.save_image) cmd += ` --save-image ${escapeArg(String(args.save_image))}`;
      if (subcommand === "set" && args.text) cmd += ` ${escapeArg(String(args.text))}`;
      return cmd;
    }
```

- [ ] **Step 3: Add tests**

Add to `tools.test.ts`:

```typescript
  it("buildCuewardCmd: calendar today", () => {
    const cmd = buildCuewardCmd("calendar", { subcommand: "today" });
    assert.equal(cmd, "cueward calendar today");
  });

  it("buildCuewardCmd: calendar list with range", () => {
    const cmd = buildCuewardCmd("calendar", { subcommand: "list", from: "2026-04-11 00:00", to: "2026-04-11 23:59" });
    assert.equal(cmd, 'cueward calendar list --from "2026-04-11 00:00" --to "2026-04-11 23:59"');
  });

  it("buildCuewardCmd: calendar create", () => {
    const cmd = buildCuewardCmd("calendar", { subcommand: "create", title: "Meeting", start: "2026-04-11 14:00", end: "2026-04-11 15:00" });
    assert.equal(cmd, 'cueward calendar create --title "Meeting" --start "2026-04-11 14:00" --end "2026-04-11 15:00"');
  });

  it("buildCuewardCmd: rejects invalid calendar subcommand", () => {
    assert.throws(() => buildCuewardCmd("calendar", { subcommand: "drop" }), /Invalid calendar subcommand/);
  });

  it("buildCuewardCmd: screenshot with ocr", () => {
    const cmd = buildCuewardCmd("screenshot", { ocr: true });
    assert.equal(cmd, "cueward screenshot --ocr");
  });

  it("buildCuewardCmd: screenshot plain", () => {
    const cmd = buildCuewardCmd("screenshot", {});
    assert.equal(cmd, "cueward screenshot");
  });

  it("buildCuewardCmd: clipboard get", () => {
    const cmd = buildCuewardCmd("clipboard", { subcommand: "get" });
    assert.equal(cmd, "cueward clipboard get");
  });

  it("buildCuewardCmd: clipboard set", () => {
    const cmd = buildCuewardCmd("clipboard", { subcommand: "set", text: "hello" });
    assert.equal(cmd, 'cueward clipboard set "hello"');
  });

  it("buildCuewardCmd: rejects invalid clipboard subcommand", () => {
    assert.throws(() => buildCuewardCmd("clipboard", { subcommand: "watch" }), /Invalid clipboard subcommand/);
  });
```

- [ ] **Step 4: Run daemon tests**

Run: `cd /Users/cyh/Development/Ryugu/daemon && npx tsx --test src/tools/tools.test.ts 2>&1 | tail -10`
Expected: all PASS

Run: `cd /Users/cyh/Development/Ryugu/daemon && npm test 2>&1 | tail -5`
Expected: all PASS

- [ ] **Step 5: Commit**

```bash
cd /Users/cyh/Development/Ryugu
git add daemon/src/tools/cueward.ts daemon/src/tools/tools.test.ts
git commit -m "feat(tools): add calendar, screenshot, clipboard to cueward wrapper"
```
