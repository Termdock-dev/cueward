---
name: cueward-agent
description: Use Cueward CLI to capture, triage, and search the user's scattered knowledge from Safari, Apple Notes, and iMessage. Trigger when the user asks about their browsing history, recent notes, messages, or wants to organize/search their daily knowledge intake. Also use when the user says things like "what did I read today", "find that article I saw", "summarize my knowledge intake", or "help me organize what I've been looking at".
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

## Important Notes

- **First run**: The user may need to grant Full Disk Access to their terminal app (System Settings > Privacy & Security > Full Disk Access). If you see permission errors, guide them through this.
- **Automation permission**: Apple Notes capture requires allowing terminal automation (System Settings > Privacy & Security > Automation).
- **JSON on stdout**: Cueward outputs clean JSON to stdout and status/warnings to stderr. Always parse stdout for data.
- **You are the LLM layer**: Cueward intentionally does not call any LLM. It captures and pre-processes locally. Summarization, insight extraction, and action item generation are your job as the Agent.
- **Auto-tagging config**: If the user wants custom tags, help them create `~/.cueward/tags.toml`:
  ```toml
  [rust]
  keywords = ["Rust", "cargo", "crate"]

  [ai]
  keywords = ["AI", "LLM", "ChatGPT", "Claude"]
  ```
