---
name: cueward-agent
description: Use when the user wants to retrieve, summarize, organize, or modify local macOS knowledge and app state through Cueward, including Safari, Notes, Reminders, Calendar, Messages, Quick Notes, Reddit, OCR, screenshots, clipboard, voice memos, stickies, or Apple Shortcuts.
---

# Cueward Agent

Cueward is the local macOS tool layer. Use it when the user needs real data or real actions from their machine instead of guesses.

Load only the reference file needed for the current request. Do not load all references by default.

## Routing

- Historical or indexed knowledge:
  Load `references/retrieval.md`
- Live Safari tabs, bookmarks, or Safari AI state:
  Load `references/safari.md`
- Notes, Quick Notes, Reminders, Calendar, OCR, screenshots, clipboard, Stickies, or Voice Memos:
  Load `references/apple-apps.md`
- Apple Shortcuts:
  Load `references/shortcuts.md`

## Core Rules

1. Prefer the narrowest command that answers the request.
2. Prefer direct reads before broad `capture`.
3. Only run `triage` when new captures need to become searchable.
4. Parse stdout as data. Treat stderr as status or warnings.
5. If behavior looks wrong on first use, suspect permissions and use `cueward doctor`.
6. For Quick Notes cleanup, prefer `quick-notes archive` over `notes move`.

