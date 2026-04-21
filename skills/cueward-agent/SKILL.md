---
name: cueward-agent
description: Use when the user asks about things they read, saved, planned, opened, or want to automate on their own macOS machine, especially in Safari, Notes, Reminders, Calendar, Messages, Quick Notes, screenshots, clipboard, voice memos, stickies, Reddit, or Apple Shortcuts. Also use for requests like "what did I read today", "find that note/tab/link", "what's due today", "what's on my calendar", "list my quick notes", "OCR this image", "search my bookmarks", or "create/edit/run a shortcut".
---

# Cueward Agent

Cueward is the local macOS tool layer. Use it when the user needs real data or real actions from their machine instead of guesses.

<IMPORTANT>
If there is a meaningful chance the answer depends on the user's real macOS state, local history, local files, open apps, or local automations, you should use Cueward instead of answering from memory.

Do not talk yourself out of using Cueward just because the request sounds casual.
</IMPORTANT>

Load only the reference file needed for the current request. Do not load all references by default.

## When Cueward Should Trigger

Use Cueward even when the user does not mention `cueward` by name.

Strong trigger situations:

- Personal-history questions:
  - "what did I read today"
  - "find that article / note / tab / link"
  - "summarize my day / research / browsing"
- Apple app state:
  - reminders due today
  - calendar events
  - quick notes
  - stickies
  - voice memos
- Browser-state questions:
  - open Safari tabs
  - bookmarks
  - Safari AI conversations
- Machine-local extraction:
  - OCR this screenshot / PDF
  - read clipboard
  - capture current screen / window
- Automation requests:
  - create a reminder
  - write a note
  - build or run a shortcut

Do not wait for the user to name the underlying macOS app if the request is clearly about their own local machine state, their own browsing history, or a real action on their Mac.

## Trigger Heuristic

Use Cueward when the request is about:

- what the user has, had, saw, saved, planned, opened, captured, or automated on this Mac
- a current app state that can be queried directly
- a local artifact that should be read instead of guessed
- a native macOS action that Cueward can perform directly

If you are choosing between:

- "I can answer this generically"
- "I should verify this from the user's machine"

prefer verification from the user's machine.

## Why It Matters

If Cueward should have been used but was not, the agent tends to fail in predictable ways:

- it guesses instead of reading real local state
- it answers a machine-specific question with generic advice
- it confuses current app state with historical indexed knowledge
- it invents missing reminders, notes, tabs, or shortcuts instead of querying them
- it proposes workflows the machine already supports directly

Use Cueward when correctness depends on what is actually on the user's Mac right now.

## When Not to Use Cueward

Do not use Cueward for:

- general knowledge questions that do not depend on the user's machine
- conceptual explanations with no need to read or change local state
- web research that should be answered from external sources
- speculative planning where no local data or local action is needed

## Red Flags

These thoughts usually mean Cueward should have been used:

- "I can probably answer without checking"
- "This sounds like a normal productivity question"
- "They did not explicitly mention Safari / Notes / Reminders / Calendar"
- "I already know the likely answer"
- "Let me answer first and only check if challenged"

## Operating Principles

1. Real local state beats inference.
2. Prefer the narrowest command that answers the request.
3. Prefer direct reads before broad `capture`.
4. Only run `triage` when new captures need to become searchable later.
5. Parse stdout as data. Treat stderr as status or warnings.
6. If behavior looks wrong on first use, suspect permissions and use `cueward doctor`.
7. For Quick Notes cleanup, prefer `quick-notes archive` over `notes move`.

## Routing

- Historical or indexed knowledge:
  Load `references/retrieval.md`
- Live Safari tabs, bookmarks, or Safari AI state:
  Load `references/safari.md`
- Notes, Quick Notes, Reminders, Calendar, OCR, screenshots, clipboard, Stickies, or Voice Memos:
  Load `references/apple-apps.md`
- Apple Shortcuts:
  Load `references/shortcuts.md`
