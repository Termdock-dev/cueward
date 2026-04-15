# Stickies CRUD Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `cueward stickies` list/create/update/delete commands on macOS using the Stickies container data files.

**Architecture:** Keep Stickies isolated as its own adapter module and top-level CLI command. Read metadata from `.SavedStickiesState`, use `UUID` as the stable `id`, and read/write content through each `<UUID>.rtfd/TXT.rtf`. Keep this batch focused on plain-text title/body CRUD only.

**Tech Stack:** Rust 2024, `clap`, plist parsing, file I/O, `textutil`, `serde`/`serde_json`, `cargo test`

---

## Chunk 1: CLI Contract

### Task 1: Define the `stickies` CLI surface

**Files:**
- Modify: `crates/cli/src/commands/mod.rs`
- Create: `crates/cli/src/commands/stickies.rs`
- Create: `crates/cli/src/commands/stickies_tests.rs`

- [ ] **Step 1: Write the failing CLI parse tests**

Cover:
- `stickies list`
- `stickies create --title --body`
- `stickies update --id --title/--body`
- `stickies delete --id`

- [ ] **Step 2: Run the CLI tests and verify they fail**

Run:
`cargo test -p cueward-cli cli_parses_stickies -- --nocapture`

Expected: FAIL because `Command::Stickies` / `StickiesAction` do not exist yet.

- [ ] **Step 3: Add the minimal CLI enums and dispatch stub**

Implement:
- `Command::Stickies { action: StickiesAction }`
- `StickiesAction::{List, Create, Update, Delete}`
- `crates/cli/src/commands/stickies.rs`

- [ ] **Step 4: Re-run the CLI tests and verify they pass**

Run:
`cargo test -p cueward-cli cli_parses_stickies -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/cli/src/commands/mod.rs crates/cli/src/commands/stickies.rs crates/cli/src/commands/stickies_tests.rs
git commit -m "test: define stickies cli contract"
```

## Chunk 2: Read Model And State Parsing

### Task 2: Implement list/read from Stickies container files

**Files:**
- Modify: `crates/adapter-macos/src/lib.rs`
- Create: `crates/adapter-macos/src/stickies.rs`

- [ ] **Step 1: Write the failing adapter tests**

Cover:
- parsing `.SavedStickiesState` yields UUID entries
- title fallback from body first line
- reading `TXT.rtf` via `textutil` path
- `update` validation requires at least one field

- [ ] **Step 2: Run the adapter tests and verify they fail**

Run:
`cargo test -p cueward-adapter-macos stickies -- --nocapture`

Expected: FAIL because the module and parser do not exist yet.

- [ ] **Step 3: Implement the read model**

Create:
- `StickiesNote { id, title, body }`
- parser for `.SavedStickiesState`
- `derive_sticky_title()`
- `read_sticky_body()`
- `list_stickies()`

Rules:
- `id` = UUID from state
- if body file missing, skip that note with explicit internal handling
- title fallback = first non-empty body line, else `Sticky <UUID-prefix>`

- [ ] **Step 4: Re-run the adapter tests**

Run:
`cargo test -p cueward-adapter-macos stickies -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/adapter-macos/src/lib.rs crates/adapter-macos/src/stickies.rs
git commit -m "feat: add stickies read model"
```

## Chunk 3: File-Based CRUD

### Task 3: Implement create/update/delete using state plist + rtfd directory

**Files:**
- Modify: `crates/adapter-macos/src/stickies.rs`
- Modify: `crates/cli/src/commands/stickies.rs`

- [ ] **Step 1: Write failing CRUD tests**

Cover:
- create writes a new UUID entry to state and creates `<UUID>.rtfd/TXT.rtf`
- update rewrites title/body for a given `id`
- delete removes both the state entry and the `UUID.rtfd`
- update without title/body fails

- [ ] **Step 2: Run the targeted tests to verify they fail**

Run:
`cargo test -p cueward-adapter-macos stickies -- --nocapture`

Expected: FAIL because file mutation helpers are missing.

- [ ] **Step 3: Implement create/update/delete**

Implement:
- `create_sticky(title, body)`
- `update_sticky(id, title, body)`
- `delete_sticky(id)`

Rules:
- mutate state plist without dropping unrelated metadata
- write RTF content through `textutil` conversion or equivalent minimal path
- not found → explicit error
- invalid id → explicit error

- [ ] **Step 4: Wire CLI dispatch**

In `crates/cli/src/commands/stickies.rs`:
- `list` prints JSON array
- `create` / `update` / `delete` print structured JSON
- failures go to stderr and exit 1

- [ ] **Step 5: Re-run targeted tests**

Run:
`cargo test -p cueward-adapter-macos stickies -- --nocapture`
`cargo test -p cueward-cli cli_parses_stickies -- --nocapture`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/adapter-macos/src/stickies.rs crates/cli/src/commands/stickies.rs
git commit -m "feat: add stickies crud operations"
```

## Chunk 4: Full Verification

### Task 4: Verify the whole feature

**Files:**
- Modify only if implementation drift requires docs alignment:
  - `docs/specs/2026-04-15-stickies-crud.md`
  - `docs/plans/2026-04-15-stickies-crud.md`

- [ ] **Step 1: Run full verification**

Run:
`cargo test`
`cargo build --release`

Expected: PASS

- [ ] **Step 2: Confirm success criteria**

Check:
- `stickies list` returns `id/title/body`
- create/update/delete all work by UUID
- missing title does not break output
- implementation uses container data files, not GUI scripting

- [ ] **Step 3: Prepare PR summary**

Call out:
- UUID-based `id`
- plain-text scope via `TXT.rtf`
- deferred items: color/position/rich text fidelity

Plan complete and saved to `docs/plans/2026-04-15-stickies-crud.md`. Ready to execute?
