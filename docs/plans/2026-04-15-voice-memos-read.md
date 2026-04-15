# Voice Memos Read Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `cueward voice-memos list/read` on macOS by reading Voice Memos metadata from the local group-container database and recordings directory.

**Architecture:** Keep Voice Memos isolated as its own adapter and CLI module. Read `ZCLOUDRECORDING` from `CloudRecordings.db`, map stable ids from `ZUNIQUEID`, and join metadata with the `Recordings/` directory using `ZPATH`. This first batch is read-only on purpose.

**Tech Stack:** Rust 2024, `clap`, rusqlite, serde/serde_json, `cargo test`

---

## Chunk 1: CLI Contract

### Task 1: Add `voice-memos` parse coverage

**Files:**
- Modify: `crates/cli/src/commands/mod.rs`
- Create: `crates/cli/src/commands/voice_memos.rs`
- Create: `crates/cli/src/commands/voice_memos_tests.rs`

- [ ] **Step 1: Write failing parse tests**

Cover:
- `voice-memos list`
- `voice-memos read --id <memo-id>`

- [ ] **Step 2: Run parse tests and verify they fail**

Run:
`cargo test -p cueward-cli cli_parses_voice_memos -- --nocapture`

Expected: FAIL because the command surface does not exist yet.

- [ ] **Step 3: Add minimal CLI enums and dispatch stub**

Implement:
- `Command::VoiceMemos { action: VoiceMemosAction }`
- `VoiceMemosAction::{List, Read { id }}`
- `crates/cli/src/commands/voice_memos.rs`

- [ ] **Step 4: Re-run parse tests**

Run:
`cargo test -p cueward-cli cli_parses_voice_memos -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/cli/src/commands/mod.rs crates/cli/src/commands/voice_memos.rs crates/cli/src/commands/voice_memos_tests.rs
git commit -m "test: define voice memos cli contract"
```

## Chunk 2: Adapter Read Model

### Task 2: Implement database-backed list/read

**Files:**
- Modify: `crates/adapter-macos/src/lib.rs`
- Create: `crates/adapter-macos/src/voice_memos.rs`

- [ ] **Step 1: Write failing adapter tests**

Cover:
- parse `ZCLOUDRECORDING` row into domain struct
- fallback title from `ZCUSTOMLABEL -> ZPATH -> ZUNIQUEID`
- timestamp conversion from Apple epoch
- read by `id`

- [ ] **Step 2: Run adapter tests and verify they fail**

Run:
`cargo test -p cueward-adapter-macos voice_memos -- --nocapture`

Expected: FAIL because module does not exist yet.

- [ ] **Step 3: Implement the read model**

Create:
- `VoiceMemoItem { id, title, duration_seconds, timestamp, path }`

Implement helpers for:
- locating `CloudRecordings.db`
- locating `Recordings/`
- mapping `ZCLOUDRECORDING`
- joining `ZPATH` to an absolute file path when present

Rules:
- `id` = `ZUNIQUEID`
- timestamp = Apple epoch -> local/RFC3339 output
- duration = `ZDURATION`
- path may be null if row exists but file is missing

- [ ] **Step 4: Add `list_voice_memos()` and `read_voice_memo(id)`**

Behavior:
- list returns all rows ordered by date desc
- read returns one row by `id`
- missing row → explicit error

- [ ] **Step 5: Re-run adapter tests**

Run:
`cargo test -p cueward-adapter-macos voice_memos -- --nocapture`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/adapter-macos/src/lib.rs crates/adapter-macos/src/voice_memos.rs
git commit -m "feat: add voice memos read model"
```

## Chunk 3: CLI Wiring

### Task 3: Wire CLI output

**Files:**
- Modify: `crates/cli/src/commands/voice_memos.rs`

- [ ] **Step 1: Implement dispatch**

Rules:
- `list` prints JSON array
- `read` prints single JSON object
- outputs use `print_external()`
- failures go to stderr and exit 1

- [ ] **Step 2: Re-run CLI and adapter targeted tests**

Run:
`cargo test -p cueward-cli cli_parses_voice_memos -- --nocapture`
`cargo test -p cueward-adapter-macos voice_memos -- --nocapture`

Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/cli/src/commands/voice_memos.rs
git commit -m "feat: add voice memos read commands"
```

## Chunk 4: Full Verification

### Task 4: Verify the batch

**Files:**
- Modify only if implementation drift requires docs alignment:
  - `docs/specs/2026-04-15-voice-memos-read.md`
  - `docs/plans/2026-04-15-voice-memos-read.md`

- [ ] **Step 1: Run full verification**

Run:
`cargo test`
`cargo build --release`

Expected: PASS

- [ ] **Step 2: Confirm success criteria**

Check:
- `voice-memos list` works
- `voice-memos read --id` works
- missing transcript does not fail the command
- implementation is read-only

- [ ] **Step 3: Prepare PR summary**

Call out:
- data source = `CloudRecordings.db` + `Recordings/`
- first batch is read-only
- delete / transcribe follow in later work

Plan complete and saved to `docs/plans/2026-04-15-voice-memos-read.md`. Ready to execute?
