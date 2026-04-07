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

For Apple Notes and Reminders operations, also allow automation:
- **System Settings > Privacy & Security > Automation** > allow your terminal to control Notes and Reminders

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
├── cli/             clap CLI (capture, triage, search, send, plan, ocr, notes)
├── adapter-macos/   Safari (SQLite), Notes (AppleScript), Messages (SQLite),
│                    Reminders (AppleScript), Vision OCR (Swift)
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
├── state.json    High watermark timestamps per source
└── tags.toml     Auto-tagging keyword rules (user-created)
```

## License

MIT
