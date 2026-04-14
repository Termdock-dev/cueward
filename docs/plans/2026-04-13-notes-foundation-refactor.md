# Notes Foundation Refactor Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor the Apple Notes internals behind `cueward send`, `cueward notes`, and `cueward quick-notes` so later attachment issues can land on clear module seams without changing the external CLI surface.

**Architecture:** Replace the single `notes.rs` file with a focused `notes/` module tree, move generic note CRUD out of `send.rs`, and keep `quick_notes.rs` limited to Quick Note-specific behavior. Preserve current output and command semantics while moving the existing image attachment pipeline into a dedicated attachments module.

**Tech Stack:** Rust Edition 2024, clap, chrono, serde, sqlite3 via CLI, AppleScript via `osascript`, cargo test

---

## Chunk 1: Split generic notes operations from `send.rs`

### Task 1: Move note CRUD into `notes::crud`

**Files:**
- Create: `crates/adapter-macos/src/notes/mod.rs`
- Create: `crates/adapter-macos/src/notes/crud.rs`
- Modify: `crates/adapter-macos/src/lib.rs`
- Modify: `crates/cli/src/commands/notes.rs`
- Modify: `crates/cli/src/commands/send.rs`
- Delete: `crates/adapter-macos/src/send.rs` only after all callers are migrated

- [ ] **Step 1: Add failing CLI parsing tests for unchanged note commands**

Cover:
- `cueward send --title T --body B --folder Cueward`
- `cueward notes update --title T --body B --folder Cueward`
- `cueward notes delete --title T --folder Cueward`
- `cueward notes move --title T --from Cueward --to Archive`

Suggested location:
- extend existing CLI test coverage in `crates/cli/src/commands/mod.rs` tests or create focused parsing tests near the command modules

- [ ] **Step 2: Run the targeted CLI tests and verify baseline behavior**

Run:
`cargo test -p cueward-cli notes -- --nocapture`

Expected: PASS for existing parsing behavior, giving a safety net before internal rewiring.

- [ ] **Step 3: Introduce `notes::crud` with the current create/update/delete/move implementations**

Implement:
- `create_note(title, body, folder)`
- `update_note(title, body, folder)`
- `delete_note(title, folder)`
- `move_note(title, from_folder, to_folder)`

Move the AppleScript logic from `send.rs` into `notes::crud` without changing behavior.

- [ ] **Step 4: Rewire CLI callers to the new module while preserving messages**

Update:
- `send` dispatch to call `cueward_adapter_macos::notes::crud::create_note(...)`
- `notes` dispatch to call `cueward_adapter_macos::notes::crud::*`

Keep success and error stderr messages unchanged.

- [ ] **Step 5: Run targeted adapter and CLI verification**

Run:
`cargo test -p cueward-adapter-macos create_note -- --nocapture`
`cargo test -p cueward-cli notes -- --nocapture`

Expected: PASS

## Chunk 2: Separate notifications from note CRUD

### Task 2: Move notification logic into `notes::notify`

**Files:**
- Create: `crates/adapter-macos/src/notes/notify.rs`
- Modify: `crates/adapter-macos/src/notes/mod.rs`
- Modify: `crates/cli/src/commands/send.rs`

- [ ] **Step 1: Write a small failing compile-level adaptation step**

Replace direct dependency on the old `send::notify()` path in CLI code so the compiler forces the new module boundary.

- [ ] **Step 2: Move notification logic into `notes::notify`**

Implement:
- `notify(title, message)`

Keep the same AppleScript and error text.

- [ ] **Step 3: Rewire `cueward send --notify`**

Ensure the preview truncation and warning message stay unchanged while the call path changes to `notes::notify::notify(...)`.

- [ ] **Step 4: Run targeted send verification**

Run:
`cargo test -p cueward-cli send -- --nocapture`

Expected: PASS or no regression in parsing coverage if tests are parser-only.

## Chunk 3: Break `notes.rs` into capture, DB, and attachments modules

### Task 3: Create the `notes/` module tree and move capture entrypoint

**Files:**
- Create: `crates/adapter-macos/src/notes/capture.rs`
- Create: `crates/adapter-macos/src/notes/db.rs`
- Create: `crates/adapter-macos/src/notes/attachments/mod.rs`
- Create: `crates/adapter-macos/src/notes/attachments/image.rs`
- Modify: `crates/adapter-macos/src/notes/mod.rs`
- Modify: `crates/adapter-macos/src/lib.rs`
- Delete: `crates/adapter-macos/src/notes.rs` only after the new module tree fully replaces it

- [ ] **Step 1: Move pure helper tests before changing behavior**

Preserve tests for:
- attachment placeholder counting
- attachment label replacement
- OCR block appending
- image attachment segment building
- media note matching heuristics

Place them beside the new owning modules.

- [ ] **Step 2: Run the targeted notes adapter tests and record the baseline**

Run:
`cargo test -p cueward-adapter-macos notes:: -- --nocapture`

Expected: PASS for the existing image attachment pipeline before or during migration.

- [ ] **Step 3: Split responsibilities without changing output**

Move:
- top-level capture flow into `capture.rs`
- SQLite path/query helpers into `db.rs`
- attachment orchestration into `attachments/mod.rs`
- current image/OCR-specific attachment logic into `attachments/image.rs`

`notes::capture(...)` should remain the public adapter entrypoint used by `MacosAdapter`.

- [ ] **Step 4: Re-run targeted notes verification**

Run:
`cargo test -p cueward-adapter-macos notes:: -- --nocapture`

Expected: PASS with unchanged user-visible JSON behavior.

## Chunk 4: Limit `quick_notes.rs` to Quick Note-specific behavior

### Task 4: Make `quick_notes.rs` depend on generic notes operations instead of duplicating them

**Files:**
- Modify: `crates/adapter-macos/src/quick_notes.rs`
- Modify: `crates/cli/src/commands/quick_notes.rs`
- Test: `crates/adapter-macos/src/quick_notes.rs`

- [ ] **Step 1: Add or preserve tests around Quick Note-only behavior**

Cover:
- unique-title enforcement
- `strip_title_block()`
- archive destination guard

- [ ] **Step 2: Replace generic note operations with `notes::crud` calls**

Update:
- create path
- delete path
- archive copy/delete path

Keep Quick Note-specific pieces local:
- SQLite query for system Quick Notes
- folder discovery
- archive polling / validation
- Quick Note body/title normalization

- [ ] **Step 3: Verify CLI behavior stays stable**

Run:
`cargo test -p cueward-cli quick_notes -- --nocapture`
`cargo test -p cueward-adapter-macos quick_notes -- --nocapture`

Expected: PASS

## Chunk 5: Full verification and cleanup

### Task 5: Run project-wide verification and align docs

**Files:**
- Modify: `docs/specs/2026-04-13-notes-attachment-pipeline-refactor.md` only if implementation drifted
- Modify: `README.md` only if internal refactor accidentally changed documented behavior

- [ ] **Step 1: Run full verification**

Run:
`cargo build --release`
`cargo test`

Expected: PASS

- [ ] **Step 2: Confirm external behavior stayed compatible**

Manual spot checks:
- `cueward send --help`
- `cueward notes --help`
- `cueward quick-notes --help`

Expected: No CLI surface drift.

- [ ] **Step 3: Review docs for semantic drift**

Only update docs if any user-visible behavior changed unexpectedly. Prefer no README change for a pure internal refactor.

- [ ] **Step 4: Remove obsolete internal paths**

Delete any superseded module file only after all callers and tests are migrated.

- [ ] **Step 5: Prepare the next batch handoff**

Document in the PR summary that the follow-up implementation target is:
- `#77` for `AttachmentSegment.kind` and unresolved fallback

Plan complete and saved to `docs/plans/2026-04-13-notes-foundation-refactor.md`. Ready to execute.
