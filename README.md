# Cueward

A CLI tool that captures scattered knowledge from macOS native sources and makes it searchable.

Cueward reads Safari history, Apple Notes, and iMessage locally — no cloud APIs, no web scraping, no third-party dependencies. It auto-tags content using keyword rules and indexes everything for fast BM25 search.

Designed as a Unix-style tool: it outputs structured JSON for AI Agents to consume.

## Install

```bash
git clone https://github.com/HCYT/cueward.git
cd cueward
cargo install --path crates/cli
```

Requires Rust 1.85+ (edition 2024).

### macOS Permissions

Cueward reads local databases that require Full Disk Access:

1. Open **System Settings > Privacy & Security > Full Disk Access**
2. Add your terminal app (Termdock, Terminal.app, iTerm2, WezTerm, etc.)

For Apple Notes, Reminders, and Calendar operations, also allow automation:
- **System Settings > Privacy & Security > Automation** > allow your terminal to control Notes, Reminders, and Calendar

## Usage

### Capture

Extract knowledge fragments from local sources:

```bash
# Everything from the last 24 hours
cueward capture --source all --since 24h

# Safari only, last 7 days
cueward capture --source safari --since 7d

# Apple Notes, last 3 hours
cueward capture --source notes --since 3h
```

### Safari

Read current Safari tabs, not just browsing history:

```bash
# List all open tabs
cueward safari tabs

# Filter by Safari profile name parsed from window title
cueward safari tabs --profile Ryugu

# Current active tab in the front window
cueward safari active

# Open a new tab
cueward safari open https://example.com

# Close current tab or a specific tab index in the front window
cueward safari close
cueward safari close --index 2

# Read current page text or a specific element
cueward safari read
cueward safari read --selector ".article-body"

# Read full HTML source
cueward safari source

# Execute JavaScript / DOM actions in the active tab
cueward safari exec "document.title"
cueward safari click "#submit"
cueward safari fill "textarea" "hello from cueward"
cueward safari wait ".result" --timeout 30
```

Outputs JSON to stdout:

```json
[
  {
    "source": "safari",
    "timestamp": "2026-04-07T03:14:16Z",
    "content": "Rust Concurrency Patterns - Blog",
    "url": "https://example.com/rust-concurrency",
    "title": "Rust Concurrency Patterns - Blog"
  },
  {
    "source": "notes",
    "timestamp": "2026-04-07T01:16:16Z",
    "content": "Meeting notes from product sync...",
    "title": "Product Sync 2026-04-07",
    "metadata": {
      "folder": "Work"
    }
  }
]
```

### Triage

Auto-tag and index captured cues:

```bash
cueward triage
```

Reads from `~/.cueward/inbox/`, applies keyword-based auto-tagging, and writes to a local BM25 index.

Configure auto-tagging in `~/.cueward/tags.toml`:

```toml
[rust]
keywords = ["Rust", "cargo", "crate", "rustc"]

[ai]
keywords = ["AI", "LLM", "ChatGPT", "Claude", "GPT"]

[finance]
keywords = ["stock", "ETF", "investment"]
```

### Search

Query the local index:

```bash
cueward search "rust concurrency" --limit 5
```

### Send

Create a digest note in Apple Notes and optionally trigger a macOS notification:

```bash
# Create a note
cueward send --title "Daily Digest" --body "Today's summary..." --folder Cueward

# With notification
cueward send --title "Daily Digest" --body "Summary" --notify

# Pipe from capture
cueward capture --source all --since 24h | cueward send --title "2026-04-07 Digest"
```

### Plan

Create a reminder in Apple Reminders:

```bash
cueward plan --title "Review PR" --notes "Check bot comments" --list Cueward
```

### Reminders

Read existing Apple Reminders:

```bash
# List all reminders
cueward reminders list

# Filter by reminders list
cueward reminders list --list Work

# Reminders due today
cueward reminders today
```

Outputs JSON with `title`, `notes`, `due_date`, `completed`, and `list_name`.

### OCR

Extract text from images or PDFs via Apple Vision Framework:

```bash
cueward ocr ~/Desktop/screenshot.png
cueward ocr ~/Documents/paper.pdf
```

Supports PNG, JPG, PDF. Languages: zh-Hant, zh-Hans, en-US, ja.

### Notes Management

Update, delete, or move Apple Notes:

```bash
# Update a note's body
cueward notes update --title "Note Title" --body "New content" --folder Cueward

# Delete a note
cueward notes delete --title "Note Title" --folder Cueward

# Move between folders
cueward notes move --title "Note Title" --from Cueward --to Archive
```

### Calendar

Query and manage Apple Calendar events:

```bash
# Today's events
cueward calendar today

# Events in a time range
cueward calendar list --from "2026-04-11 09:00" --to "2026-04-11 18:00"

# Filter by calendar
cueward calendar list --calendar Work

# Create an event
cueward calendar create --title "Team Sync" --start "2026-04-12 14:00" --end "2026-04-12 15:00" --calendar Work --location "Google Meet" --notes "Weekly sync"

# Delete an event (matches by title + start time)
cueward calendar delete --title "Team Sync" --start "2026-04-12 14:00"
```

Datetime format: ISO 8601 (`2026-04-11T14:00:00`) or `YYYY-MM-DD HH:MM`.

### Screenshot

Capture a screenshot, optionally with OCR:

```bash
# Capture main screen
cueward screenshot

# With OCR text extraction
cueward screenshot --ocr

# Specific display (1=main, 2=secondary, 3=third)
cueward screenshot --display 2

# Custom output path
cueward screenshot --output ~/Desktop/shot.png --ocr
```

### Clipboard

Read and write the system clipboard:

```bash
# Read clipboard (text or image)
cueward clipboard get

# Save clipboard image to a specific path
cueward clipboard get --save-image ~/Desktop/clip.png

# Write text to clipboard
cueward clipboard set "Hello from cueward"
```

Text content returns JSON with `"type": "text"`. Image content is saved as PNG and returns `"type": "image"` with the file path.

### Quick Notes

List, update, archive, and delete system Quick Notes (快速備忘錄):

```bash
# List all Quick Notes
cueward quick-notes list

# Update a Quick Note's body
cueward quick-notes update --title "Note Title" --body "New content"

# Delete a Quick Note
cueward quick-notes delete --title "Note Title"

# Archive a Quick Note into a regular note, then remove it from Quick Notes
cueward quick-notes archive --title "Note Title" --to Archive

# Create a note in the Quick Notes folder
cueward quick-notes create --title "Title" --body "Content"
```

Quick Notes are identified by the system `ZISSYSTEMPAPER` flag — notes created via the macOS Quick Note gesture (hot corner, Apple Pencil, etc.). `list`, `update`, and `delete` operate on these system-tagged notes regardless of which folder they reside in. `create` places a regular note in the "Quick Notes" folder but does not mark it as a system Quick Note.

`archive` is the cleanup workflow for real Quick Notes: it copies the note into a regular destination folder, waits for the new note to appear, and deletes the original Quick Note so it disappears from the Quick Notes smart view. This preserves link URLs, but Apple Notes rich-link cards may be flattened into normal links in the archived copy.


## Agent Integration

Cueward outputs structured JSON — it does not call any LLM. The LLM layer is your Agent's responsibility.

### Pipe to an Agent

```bash
# Claude Code
cueward capture --source all --since 24h | claude --print "Summarize my knowledge intake today"

# Gemini CLI
cueward capture --source all --since 24h | gemini "Group these by topic and highlight action items"
```

### As a Skill

A reference skill for Claude Code is included in `skills/cueward-agent/`. Copy it to your skills directory to teach Claude how to use Cueward automatically:

```bash
mkdir -p ~/.claude/skills/ && cp -r skills/cueward-agent ~/.claude/skills/
```

## Architecture

```
crates/
├── core/            Cue struct, PlatformAdapter trait, BM25 index, auto-tagger
├── cli/             clap CLI (capture, triage, search, send, plan, reminders, ocr, safari,
│                    notes, quick-notes, calendar, screenshot, clipboard)
├── adapter-macos/   Safari (SQLite), Notes (AppleScript), Messages (SQLite),
│                    Safari current tabs (AppleScript), Reminders write/read (AppleScript),
│                    Calendar (AppleScript), Vision OCR (Swift), Screenshot (screencapture),
│                    Clipboard (pbpaste/pbcopy)
└── adapter-windows/ Reserved for future cross-platform support
```

- **Core Engine + Adapter Pattern**: Platform-specific code is isolated in adapters. Core logic is platform-agnostic.
- **Native First**: Direct SQLite reads, AppleScript, and Vision Framework. No web scraping, no browser automation.
- **Privacy**: All data extraction happens locally. Nothing leaves your machine.

## Data Storage

```
~/.cueward/
├── inbox/        Captured cues awaiting triage
├── processed/    Triaged cues (moved from inbox)
├── index/        Tantivy BM25 search index
├── cache/
│   ├── ocr/          OCR result cache (by SHA256)
│   ├── screenshots/  Screenshot captures
│   └── clipboard/    Clipboard image captures
├── state.json    High watermark timestamps per source
└── tags.toml     Auto-tagging keyword rules (user-created)
```

## License

MIT
