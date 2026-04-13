---
name: cueward-agent
description: Use Cueward CLI to capture, triage, search, and manage the user's scattered knowledge from Safari, Apple Notes, and iMessage. Also supports Safari tabs/bookmarks/AI automation, OCR (images/PDFs), creating Notes and Reminders, reading Reminders, managing notes (update/delete/move), and Quick Notes (快速備忘錄) operations including archive-to-folder cleanup. Trigger when the user asks about their browsing history, recent notes, messages, Quick Notes, Safari tabs/bookmarks, wants to organize knowledge, create a digest, set reminders from captured content, inspect Safari AI conversations, extract text from images/PDFs, or manage Apple Notes. Also use when the user says things like "what did I read today", "find that article I saw", "summarize my knowledge intake", "help me organize what I've been looking at", "create a reminder for this", "write a summary note", "OCR this screenshot", "list my quick notes", "what's in my quick notes", "show my Safari tabs", or "search my bookmarks".
---

# Cueward Agent Skill

Cueward is a local CLI tool that extracts knowledge fragments from macOS native sources (Safari, Apple Notes, iMessage), auto-tags them, indexes them for search, and outputs structured JSON for you to process.

Your role as the Agent is to call Cueward commands, interpret the JSON output, and provide the user with summaries, insights, or answers based on their captured knowledge.

## Commands

### 1. Capture

Extracts raw knowledge fragments from local sources.

```bash
cueward capture --source <safari|notes|messages|all> --since <duration>
```

- `--source`: Which source to capture from. Default: `all`
- `--since`: Time window. Supports `h` (hours), `d` (days), `m` (minutes). Default: `24h`
- Outputs JSON to stdout, status messages to stderr
- Also saves to `~/.cueward/inbox/` for later triage

**Output format** (JSON array of Cue objects):
```json
[
  {
    "source": "safari",
    "timestamp": "2026-04-07T03:14:16Z",
    "content": "Page title or note body",
    "url": "https://...",
    "title": "Page title",
    "tags": ["ai", "rust"],
    "metadata": {
      "folder": "Notes",
      "direction": "received",
      "sender": "+886..."
    }
  }
]
```

Fields vary by source:
- **Safari**: always has `url` and `title`
- **Notes**: has `title`, `metadata.folder`, no `url`
- **Messages**: has `metadata.sender`, `metadata.direction`, no `url` or `title`

### 2. Triage

Processes inbox cues: auto-tags with keyword rules and indexes for search.

```bash
cueward triage
```

- Reads from `~/.cueward/inbox/`
- Auto-tags using `~/.cueward/tags.toml` (if present)
- Indexes into local BM25 search index
- Moves processed files to `~/.cueward/processed/`

### 3. Search

Queries the local BM25 index.

```bash
cueward search "<query>" --limit <N>
```

- Returns JSON objects to stdout, one per line
- Best for English keyword searches; Chinese content works but tokenization is basic
- Search results include: source, timestamp, title, content, url, tags — but NOT metadata fields (sender, folder, direction). For full metadata, use `capture` output directly

### 4. Send

Create a note in Apple Notes and optionally send a macOS notification.

```bash
cueward send --title "Daily Digest" --body "Summary content..." --folder Cueward --notify
```

- `--folder`: Target Notes folder (auto-created if missing). Default: `Cueward`
- `--notify`: Also trigger a macOS notification
- Body can be piped via stdin if `--body` is omitted

### 5. Safari

Read live Safari state and automate the current page or a matched tab.

```bash
# List open tabs
cueward safari tabs

# Filter to a Safari profile
cueward safari tabs --profile Ryugu

# Read the active page or a specific selector
cueward safari read
cueward safari read --selector ".article-body"

# Read a specific tab by URL/title match
cueward safari read --tab "ChatGPT" --profile Ryugu

# Run JavaScript in the current tab
cueward safari exec "document.title"
```

- Use these commands when the user wants the current Safari context, not historical capture data
- `--profile` targets a Safari profile parsed from the window title
- `--tab` can match by index or URL/title substring depending on the subcommand

### 6. Safari Bookmarks

Inspect and manage Safari bookmarks, including nested folders and profile roots.

```bash
# List root bookmarks or a profile root
cueward safari bookmarks list
cueward safari bookmarks list --profile Ryugu

# Traverse nested folders inside a profile
cueward safari bookmarks list --profile Ryugu --folder "Work/AI Tools"

# Recursive search
cueward safari bookmarks search "claude" --profile Ryugu --folder "Work"

# Add or delete by exact title + URL fingerprint inside a folder
cueward safari bookmarks add --title "Claude" --url "https://claude.ai" --profile Ryugu --folder "Work/AI Tools"
cueward safari bookmarks delete --title "Claude" --url "https://claude.ai" --profile Ryugu --folder "Work/AI Tools"
```

- Folder paths use `/` as the separator
- In the current implementation, folder titles containing `/` are not supported
- `delete` must include both `--title` and `--url`

### 7. Safari AI

Drive Safari-based AI providers such as Gemini and ChatGPT through the CLI.

```bash
# Send a prompt
cueward safari ai --provider gemini prompt --prompt "台灣 AI 產業分析"

# Use a provider with a specific Safari profile
cueward safari ai --provider gemini --profile Ryugu list

# Read a saved conversation
cueward safari ai --provider gemini read https://gemini.google.com/app/abc123

# Save generated images
cueward safari ai --provider chatgpt save-images https://chatgpt.com/c/abc123 --output ~/Downloads
```

- Use this when the user wants to inspect or continue a Safari AI workflow they already ran in the browser
- Prefer `list` then `read` when the user asks about prior AI conversations
- Gemini supports extra workflow commands such as `mode`, `poll`, and `save-media`

### 8. Plan

Create a reminder in Apple Reminders.

```bash
cueward plan --title "Review PR" --notes "Check bot comments" --list Cueward
```

- `--list`: Reminders list (auto-created if missing). Default: `Cueward`

### 9. Reminders

Read existing Apple Reminders.

```bash
cueward reminders list
cueward reminders list --list Work
cueward reminders today
```

- Use this when the user asks what they planned already, what is due today, or wants a reminder digest

### 10. OCR

Extract text from images or PDFs using Apple Vision Framework.

```bash
cueward ocr <path_to_image_or_pdf>
```

- Supports PNG, JPG, PDF
- PDF: uses native text layer first, falls back to Vision OCR for scanned pages
- Languages: zh-Hant, zh-Hans, en-US, ja
- Outputs standard Cue JSON (source: "ocr")

### 11. Notes Management

Update, delete, or move Apple Notes.

```bash
# Update a note's body
cueward notes update --title "Note Title" --body "New content" --folder Cueward

# Delete a note
cueward notes delete --title "Note Title" --folder Cueward

# Move a note between folders
cueward notes move --title "Note Title" --from Cueward --to Archive
```

These commands find notes by exact title match within the specified folder.

### 12. Quick Notes

List, update, archive, and delete system Quick Notes (快速備忘錄). Quick Notes are notes created via the macOS Quick Note gesture and tagged with a system flag — they may reside in any folder.

```bash
# List all Quick Notes
cueward quick-notes list

# Update a Quick Note's body (finds by title across all folders)
cueward quick-notes update --title "Note Title" --body "New content"

# Delete a Quick Note
cueward quick-notes delete --title "Note Title"

# Archive a Quick Note into a regular note and remove it from Quick Notes
cueward quick-notes archive --title "Note Title" --to Archive

# Create a note in the Quick Notes folder (not a system Quick Note)
cueward quick-notes create --title "Title" --body "Content"
```

- `list` outputs a JSON array of `{"title": "...", "folder": "..."}` objects to stdout
- `update` and `delete` locate the note by title using the system Quick Note flag — no folder needed
- `archive` requires a unique title among current Quick Notes, copies the note into a regular folder, then deletes the original Quick Note so it leaves the Quick Notes smart view
- `create` places a regular note in the "Quick Notes" folder; it will NOT appear in the system Quick Notes smart folder (快速備忘錄) unless created via macOS gesture
- `archive` preserves plain text and URLs, but Apple Notes rich-link card styling may flatten into a normal link in the archived copy

## Workflow Patterns

### Daily knowledge digest

When the user asks "what did I look at today" or "summarize my day":

```bash
cueward capture --source all --since 24h
```

Then summarize the JSON output, grouping by source and topic. Deduplicate similar URLs. Highlight anything with action items.

### Find something the user saw before

When the user asks "I saw an article about X" or "find that link about Y":

1. First try search (fast, uses index):
   ```bash
   cueward search "X" --limit 5
   ```
2. If search returns nothing, capture fresh and grep:
   ```bash
   cueward capture --source safari --since 7d
   ```
   Then filter the JSON output for relevant content.

### Triage and organize

When the user wants to organize their knowledge:

```bash
cueward capture --source all --since 7d
cueward triage
```

Then read the indexed results or the processed JSON to provide a structured overview.

### Capture, summarize, and archive

Full pipeline: capture → summarize → write digest → clean up:

```bash
# 1. Capture today's knowledge
cueward capture --source all --since 24h

# 2. (You summarize the JSON output)

# 3. Write digest to Notes
cueward send --title "2026-04-07 Digest" --body "<your summary>" --folder Cueward --notify

# 4. Create reminders for action items
cueward plan --title "Follow up on X" --notes "From today's capture"

# 5. Archive processed notes
cueward notes move --title "Old Note" --from Notes --to Archive
```

### OCR a document

When the user shares an image or PDF:

```bash
cueward ocr ~/Desktop/screenshot.png
```

Then summarize the extracted text or answer questions about it.

### Quick Notes review

When the user asks about their Quick Notes or wants to review jotted-down ideas:

```bash
cueward quick-notes list
```

Then summarize the Quick Notes, group by folder, or help the user decide what to keep, archive, or delete.

When the user wants Quick Notes to truly disappear from the Quick Notes smart view after triage, prefer:

```bash
cueward quick-notes archive --title "Note Title" --to Archive
```

Do not use `cueward notes move` for this cleanup workflow. Moving a Quick Note between folders does not reliably remove its system Quick Note identity.

## Important Notes

- **First run**: The user may need to grant Full Disk Access to their terminal app (System Settings > Privacy & Security > Full Disk Access). If you see permission errors, guide them through this.
- **Automation permission**: Apple Notes capture requires allowing terminal automation in System Settings > Privacy & Security > Automation.
- **JSON on stdout**: Cueward outputs clean JSON to stdout and status/warnings to stderr. Always parse stdout for data.
- **You are the LLM layer**: Cueward intentionally does not call any LLM. It captures and pre-processes locally. Summarization, insight extraction, and action item generation are your job as the Agent.
- **Auto-tagging config**: If the user wants custom tags, help them create `~/.cueward/tags.toml`:
  ```toml
  [rust]
  keywords = ["Rust", "cargo", "crate"]

  [ai]
  keywords = ["AI", "LLM", "ChatGPT", "Claude"]
  ```
