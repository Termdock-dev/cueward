# Cueward

Local memory and automation for AI agents on macOS.

Cueward is a Unix-style CLI for agents that need structured access to native macOS data and actions. It reads Safari, Notes, Reminders, Calendar, Messages, Voice Memos, Stickies, Quick Notes, and Apple Shortcuts locally, then returns machine-friendly JSON that agents can actually use.

It is designed for agent workflows first:

- Native macOS reach: SQLite reads, AppleScript, EventKit, Vision OCR, and Shortcuts integration
- Agent-friendly output: structured JSON instead of chatty terminal prose
- Local-first privacy: no cloud APIs, no scraping proxy, no third-party data backend
- Practical automation: diagnose permissions with `cueward doctor`, then read, capture, search, and act

Common use cases:

- Give an agent a searchable local memory layer for what you read, saved, and wrote on macOS
- Build Shortcuts programmatically from CLI or spec files
- Read reminders and calendar events fast enough for background agents and daily briefings
- Capture Safari, Notes, screenshots, clipboard, and OCR results into a local index

Most first-time macOS integrations require system permissions before they work. If a command fails immediately on first use, check Privacy & Security settings first, grant the needed access, then run it again.

## Install

Install the latest published release from crates.io:

```bash
cargo install cueward-cli --locked
```

To build from the local repo instead:

```bash
git clone https://github.com/HCYT/cueward.git
cd cueward
cargo install --path crates/cli
```

Requires Rust 1.85+ (edition 2024).

## What's New in 0.3.0

Cueward `0.3.0` adds several major capabilities and reliability improvements:

- Shortcuts CLI: create, run, rename, move, apply/export spec, Share Sheet setup, and action editing
- `cueward doctor`: audit Full Disk Access, Automation, and optional Safari live probes before using integrations
- Reminders reads moved to EventKit-first with AppleScript fallback, removing the previous multi-second read bottleneck on supported setups
- Calendar reads moved to EventKit-first with AppleScript fallback
- Notes attachment support expanded, including drawing attachments and richer structured attachment enrichment

### macOS Permissions

Cueward reads local databases that require Full Disk Access:

1. Open **System Settings > Privacy & Security > Full Disk Access**
2. Add your terminal app (Termdock, Terminal.app, iTerm2, WezTerm, etc.)

For Apple Notes, Reminders, and Calendar operations, also allow automation:
- **System Settings > Privacy & Security > Automation** > allow your terminal to control Notes, Reminders, and Calendar

Some integrations may additionally require:
- **Accessibility / 輔助使用** for UI scripting style automations
- app-specific data access via **Full Disk Access** when reading container files

### Calendar / Reminders Read Access

As of `0.3.0`, Cueward prefers EventKit for `reminders` and `calendar` read commands because it is dramatically faster and more reliable than app scripting.

- `cueward reminders list`
- `cueward reminders today`
- `cueward reminders list --due-tomorrow`
- `cueward calendar list`
- `cueward calendar today`

For Reminders, allow the terminal app to read reminders when macOS prompts for access.

For Calendar, newer macOS versions may expose more than one permission level. Depending on your system language/version, you may see labels similar to:

- `取用` / `僅寫入`
- `完整取用`

If Calendar only has write-only access, Cueward will fall back to AppleScript for reads. That keeps commands working, but `calendar list` / `calendar today` can be much slower on some calendars. For the best performance, grant **Calendar full access / 完整取用** to your terminal app.

## Usage

### Discover Commands

Use the built-in Clap help to explore the CLI surface:

```bash
# Top-level command list
cueward --help

# Subcommand-specific help
cueward notes --help
cueward reminders --help
cueward safari --help

# Alternative form
cueward help doctor
cueward help notes
```

This is the fastest way to see the current command tree and flags, especially as new integrations land.

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
cueward safari tabs --profile Work

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

# Target a specific tab by index or URL/title match
cueward safari read --tab "gemini.google.com" --profile Work
cueward safari exec "document.title" --tab 2
cueward safari source --tab "ChatGPT"

# Scroll the page
cueward safari scroll down
cueward safari scroll up --amount 1000
cueward safari scroll top
cueward safari scroll bottom --profile Work

# Close multiple tabs by profile or URL pattern
cueward safari close-tabs --profile Work --url "gemini.google.com"
cueward safari close-tabs --profile Work  # close all tabs in profile

# List bookmark/folder items from the Safari bookmarks root
cueward safari bookmarks list

# Scope bookmarks to a specific Safari profile folder
cueward safari bookmarks list --profile Work

# Traverse nested bookmark folders inside a profile
cueward safari bookmarks list --profile Work --folder "Projects/AI Tools"

# Folder paths use "/" as the separator; folder titles containing "/" are not supported
# in this first version

# Search bookmarks recursively from the root or a profile folder
cueward safari bookmarks search "claude"
cueward safari bookmarks search "claude" --profile Work --folder "Projects"

# Add a bookmark into a nested folder inside a profile
cueward safari bookmarks add --title "Claude" --url "https://claude.ai" --profile Work --folder "Projects/AI Tools"

# Delete by exact title + URL within a profile folder
cueward safari bookmarks delete --title "Claude" --url "https://claude.ai" --profile Work --folder "Projects/AI Tools"
```

### Safari AI

Control web-based AI providers (Gemini, ChatGPT) via Safari automation. Uses URL navigation and `execCommand` — no fragile DOM clicking, no focus stealing.

```bash
# Send a prompt (general chat)
cueward safari ai --provider gemini prompt --prompt "explain quantum computing"

# Switch to a specific mode first
cueward safari ai --provider gemini prompt --prompt "a cat on a keyboard" --mode image

# Deep Research with auto-confirm
cueward safari ai --provider gemini prompt --prompt "台灣 AI 產業分析" --mode deep-research --auto-confirm

# Switch mode only (no prompt)
cueward safari ai --provider gemini mode deep-research

# List conversations from sidebar
cueward safari ai --provider gemini list

# Read a conversation's text content (reports, chat history)
cueward safari ai --provider gemini read https://gemini.google.com/app/abc123

# Poll an in-progress Deep Research
cueward safari ai --provider gemini poll --timeout 300

# Save AI-generated images as PNG
cueward safari ai --provider gemini save-images https://gemini.google.com/app/abc123 --output ~/Downloads

# Download video/music via browser (triggers Safari native download)
cueward safari ai --provider gemini save-media https://gemini.google.com/app/abc123

# Use a specific Safari profile
cueward safari ai --provider gemini --profile Work list
```

Supported Gemini modes: `deep-research`, `image`, `video`, `music`.

### Reddit

Read Reddit via public `old.reddit.com/*.json` endpoints. These commands do not use Safari automation.

```bash
# Read a subreddit feed
cueward reddit feed rust
cueward reddit feed r/rust --limit 50

# Read a post plus top-level comments
cueward reddit post https://www.reddit.com/r/rust/comments/abc123/example_title/

# Search posts globally or inside one subreddit
cueward reddit search "async rust"
cueward reddit search "async rust" --subreddit r/rust --limit 25
```

Repeated scans may return status metadata such as `fresh`, `unchanged`, `skipped`, `warning`, or `deleted`, with `data` omitted when the target is skipped or confirmed deleted.

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

Read and manage Apple Reminders:

```bash
# List all reminders
cueward reminders list

# Filter by reminders list
cueward reminders list --list Work

# Reminders due today
cueward reminders today

# Create a reminder
cueward reminders create --title "Review PR" --due "2026-04-22 10:00" --list Cueward --notes "Check review threads"

# Update a reminder by id or title
cueward reminders update --id x-apple-reminder://123 --new-title "Review PR #114" --priority 5
cueward reminders update --title "Review PR" --list Archive

# Mark complete
cueward reminders complete --title "Review PR"

# Delete
cueward reminders delete --title "Review PR"
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
# Create a note
cueward notes create --title "Daily Digest" --body "Summary..." --folder Cueward

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
cueward calendar delete --title "Team Sync" --start "2026-04-12 14:00" --calendar Work

# Update an event
cueward calendar update --title "Team Sync" --calendar Work --new-start "2026-04-12 14:30" --new-end "2026-04-12 15:30"
```

Datetime format: ISO 8601 (`2026-04-11T14:00:00`) or `YYYY-MM-DD HH:MM`.

### Shortcuts

Create and manage Apple Shortcuts, including declarative spec workflows:

```bash
# List shortcuts
cueward shortcuts list

# Create a blank shortcut
cueward shortcuts create "Clean URL Share"

# Show one shortcut as a high-level YAML-like spec
cueward shortcuts show --name "Clean URL Share"

# Set accepted input and attach Share Sheet surface
cueward shortcuts input-type --name "Clean URL Share" url
cueward shortcuts surface --name "Clean URL Share" share-sheet
cueward shortcuts surface --name "Clean URL Share" library-root

# Append actions incrementally
cueward shortcuts add-text --name "Clean URL Share" --value "hello"
cueward shortcuts add-get-urls --name "Clean URL Share" --from extension-input --output urls
cueward shortcuts add-get-text --name "Clean URL Share" --from urls --output url_text
cueward shortcuts add-replace-text --name "Clean URL Share" --from text --find "hello" --replace "world"
cueward shortcuts add-copy-to-clipboard --name "Clean URL Share" --from text_2
cueward shortcuts add-share --name "Clean URL Share" --from text_2

# Control flow
cueward shortcuts add-if --name "Clean URL Share" --input text --value world --then-actions then.yaml
cueward shortcuts add-repeat --name "Clean URL Share" --input urls --body-actions repeat.yaml

# Spec-based workflow
cueward shortcuts validate-spec clean-url-share.yaml
cueward shortcuts apply clean-url-share.yaml
cueward shortcuts export-spec --name "Clean URL Share"

# Rename, move, and run
cueward shortcuts rename --name "Clean URL Share" "Clean URL Share v2"
cueward shortcuts move --name "Clean URL Share v2" "Utilities"
cueward shortcuts run --name "Clean URL Share v2"
```

Selector-based commands generally accept either `--name` or `--id`.

### Screenshot

Capture a screenshot, optionally with OCR:

```bash
# Capture main screen
cueward screenshot

# With OCR text extraction
cueward screenshot --ocr

# Specific display (1=main, 2=secondary, 3=third)
cueward screenshot --display 2

# List capturable windows
cueward screenshot windows

# Capture a specific window
cueward screenshot window --id 12345

# Capture a specific window with OCR
cueward screenshot window --id 12345 --ocr

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

### Doctor

Run a read-only macOS preflight before using integrations that depend on permissions:

```bash
# Human-readable summary
cueward doctor

# Machine-readable report
cueward doctor --json

# Opt-in Safari JavaScript probe
cueward doctor --live-safari
cueward doctor --json --live-safari
```

`doctor` checks:

- filesystem / Full Disk Access access to the current local data sources
- Apple Events / Automation access for Notes, Reminders, Calendar, and Safari
- an optional Safari JavaScript probe that reuses the normal Safari guard path

The JSON output includes stable check IDs such as `fda.messages.chat_db`, `automation.notes`, and `live.safari.js`.

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

### Voice Memos

Read Voice Memos metadata from the local shared database:

```bash
# List all voice memos
cueward voice-memos list

# Read one voice memo by id
cueward voice-memos read --id F45D4751-183C-4032-99F7-F1FE1F541BA2
```

Outputs JSON with `id`, `title`, `duration_seconds`, `timestamp`, and `path`.

### Stickies

Manage Stickies notes from the desktop:

```bash
# List notes
cueward stickies list

# Create a note
cueward stickies create --title "Temp" --body "Remember this"

# Update a note
cueward stickies update --id sticky-1 --title "Updated title"

# Delete a note
cueward stickies delete --id sticky-1
```

Use `cueward stickies --help` to inspect the geometry and color flags for `create` / `update`.


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
├── core/               Cue types, adapter trait, inbox/state/index, shortcuts spec model
├── cli/
│   ├── main.rs         CLI entrypoint
│   └── commands/       Per-command clap enums, dispatch, and parse tests
├── adapter-macos/
│   ├── applescript.rs      Shared AppleScript helpers
│   ├── bookmarks/          Safari bookmarks CRUD + plist tree operations
│   ├── calendar.rs         Apple Calendar CRUD + AppleScript fallback
│   ├── calendar_eventkit.rs EventKit-backed calendar reads
│   ├── clipboard.rs        Clipboard read / write
│   ├── doctor/             Full Disk Access / Automation diagnostics
│   ├── messages.rs         iMessage capture
│   ├── notes/              Apple Notes CRUD, capture, DB reads, attachments
│   ├── ocr.rs              Vision OCR
│   ├── plan.rs             Reminder creation shortcut command
│   ├── quick_notes.rs      Quick Notes workflows
│   ├── reddit/             Reddit JSON API reads and scan-state integration
│   ├── reminders.rs        Apple Reminders read / write + AppleScript fallback
│   ├── reminders/eventkit.rs EventKit-backed reminder reads
│   ├── safari/             Tabs, history, AI providers, social feeds
│   ├── safari_guard.rs     Shared Safari rate limit + file lock guard
│   ├── scan_state.rs       Shared polling / target state tracking
│   ├── screenshot/         Screen/window capture + OCR integration
│   ├── shortcuts/          Shortcuts DB compiler, installer, and tests
│   ├── stickies/           Stickies CRUD, geometry, color, state
│   └── voice_memos.rs      Voice Memos metadata reads
└── adapter-windows/    Reserved for future cross-platform support
```

- **Core Engine + Adapter Pattern**: Platform-specific code is isolated in adapters. Core logic is platform-agnostic.
- **Native First**: Direct SQLite reads, AppleScript, EventKit, and Vision Framework. No cloud APIs and no browser-driving frameworks in the normal data path.
- **Privacy**: All data extraction happens locally. Nothing leaves your machine.

## Data Storage

```
~/.cueward/
├── inbox/            Captured cues awaiting triage
├── processed/        Triaged cues moved out of inbox
├── index/            Tantivy BM25 search index and lock files
├── cache/
│   ├── ocr/          OCR result cache keyed by SHA256
│   ├── screenshots/  Screenshot captures
│   └── clipboard/    Clipboard image captures
├── state.json        High watermark timestamps and scan target state
├── tags.toml         Auto-tagging keyword rules
└── lock.json         Safari automation lock / rate-limit coordination
```

Additional app- or tool-specific scratch directories may appear under `~/.cueward/` over time, but the paths above are the stable managed data layout that Cueward itself depends on.

## License

MIT
