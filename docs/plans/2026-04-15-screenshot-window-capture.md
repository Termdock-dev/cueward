# Screenshot Window Capture Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `screenshot windows` and `screenshot window --id <id>` while keeping the existing full-screen screenshot behavior intact.

**Architecture:** Split the current screenshot module into focused capture and window-list submodules. Keep OCR and output-path handling shared, and add a dedicated macOS window enumeration path backed by `CGWindowListCopyWindowInfo` for listing capturable windows.

**Tech Stack:** Rust 2024, `clap`, macOS `screencapture`, CoreGraphics window metadata via `swift -e`, `serde`/`serde_json`, `cargo test`

---

## Chunk 1: Reshape Screenshot Module Boundaries

### Task 1: Split screenshot logic into capture and windows concerns

**Files:**
- Move: `crates/adapter-macos/src/screenshot.rs` -> `crates/adapter-macos/src/screenshot/mod.rs`
- Create: `crates/adapter-macos/src/screenshot/capture.rs`
- Create: `crates/adapter-macos/src/screenshot/windows.rs`
- Create: `crates/adapter-macos/src/screenshot/tests.rs`
- Modify: `crates/adapter-macos/src/lib.rs`

- [ ] **Step 1: Write failing adapter tests for the refactor seam**

Cover:
- display validation still works
- output path validation still works
- screenshot result shape remains unchanged

- [ ] **Step 2: Run targeted screenshot tests and verify they fail**

Run:
`cargo test -p cueward-adapter-macos screenshot -- --nocapture`

Expected: FAIL because the new module layout does not exist yet.

- [ ] **Step 3: Move existing full-screen capture flow into submodules**

Keep:
- current `screencapture -D <display>` path
- current OCR flow
- current cache directory logic

- [ ] **Step 4: Re-run targeted tests and verify they pass**

Run:
`cargo test -p cueward-adapter-macos screenshot -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/adapter-macos/src/screenshot crates/adapter-macos/src/lib.rs
git commit -m "refactor: split screenshot capture and window helpers"
```

## Chunk 2: Extend CLI Contract

### Task 2: Add `windows` and `window --id` CLI entrypoints

**Files:**
- Modify: `crates/cli/src/commands/mod.rs`
- Modify: `crates/cli/src/commands/screenshot.rs`
- Create or modify: `crates/cli/src/commands/screenshot_tests.rs`

- [ ] **Step 1: Write failing CLI parse tests**

Cover:
- `cueward screenshot`
- `cueward screenshot windows`
- `cueward screenshot window --id 123`
- `cueward screenshot window --id 123 --ocr`
- `cueward screenshot window --id 123 --output out.png`

- [ ] **Step 2: Run targeted CLI tests and verify they fail**

Run:
`cargo test -p cueward-cli screenshot_tests -- --nocapture`

Expected: FAIL because the new command shapes are not defined yet.

- [ ] **Step 3: Implement the new CLI contract**

Rules:
- keep bare `cueward screenshot` as existing full-screen mode
- add `screenshot windows`
- add `screenshot window --id <id>`

- [ ] **Step 4: Re-run targeted CLI tests and verify they pass**

Run:
`cargo test -p cueward-cli screenshot_tests -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/cli/src/commands/mod.rs crates/cli/src/commands/screenshot.rs crates/cli/src/commands/screenshot_tests.rs
git commit -m "test: define screenshot window capture cli"
```

## Chunk 3: Implement Capturable Window Listing

### Task 3: Add `screenshot windows`

**Files:**
- Modify: `crates/adapter-macos/src/screenshot/windows.rs`
- Modify: `crates/adapter-macos/src/screenshot/mod.rs`
- Modify: `crates/adapter-macos/src/screenshot/tests.rs`
- Modify: `crates/cli/src/commands/screenshot.rs`

- [ ] **Step 1: Write failing adapter tests for window parsing/filtering**

Cover:
- parse window metadata into typed structs
- reject windows with zero width/height
- reject offscreen / transparent / high-layer noise windows
- sort frontmost windows first

- [ ] **Step 2: Run targeted tests and verify they fail**

Run:
`cargo test -p cueward-adapter-macos screenshot -- --nocapture`

Expected: FAIL because window enumeration/parsing is not implemented yet.

- [ ] **Step 3: Implement typed window metadata**

Add:
- `CapturableWindow`
- `WindowBounds`
- parser from `swift -e` JSON payload
- filtering + sorting helpers

- [ ] **Step 4: Wire `screenshot windows` command**

Output JSON only:
- `window_id`
- `app`
- `title`
- `owner_pid`
- `is_frontmost`
- `bounds`

- [ ] **Step 5: Re-run targeted tests and verify they pass**

Run:
`cargo test -p cueward-adapter-macos screenshot -- --nocapture`
`cargo test -p cueward-cli screenshot_tests -- --nocapture`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/adapter-macos/src/screenshot crates/cli/src/commands/screenshot.rs crates/cli/src/commands/screenshot_tests.rs
git commit -m "feat: add screenshot window listing"
```

## Chunk 4: Implement `window --id` Capture

### Task 4: Add screenshot capture by window id

**Files:**
- Modify: `crates/adapter-macos/src/screenshot/capture.rs`
- Modify: `crates/adapter-macos/src/screenshot/windows.rs`
- Modify: `crates/adapter-macos/src/screenshot/mod.rs`
- Modify: `crates/adapter-macos/src/screenshot/tests.rs`
- Modify: `crates/cli/src/commands/screenshot.rs`

- [ ] **Step 1: Write failing tests for window-id capture**

Cover:
- command builder uses `screencapture -l <id>`
- invalid / missing window id returns clear error
- OCR path still works in window mode
- output path still works in window mode

- [ ] **Step 2: Run targeted tests and verify they fail**

Run:
`cargo test -p cueward-adapter-macos screenshot -- --nocapture`

Expected: FAIL because window-id capture does not exist yet.

- [ ] **Step 3: Implement `capture_window()`**

Rules:
- require valid `window_id`
- use `screencapture -x -l <id>`
- share output path and OCR logic with full-screen capture
- reject window ids that are not in the capturable list

- [ ] **Step 4: Wire CLI dispatch**

Behavior:
- `screenshot windows` lists windows
- `screenshot window --id` captures one window
- bare `screenshot` remains full-screen

- [ ] **Step 5: Re-run targeted tests and verify they pass**

Run:
`cargo test -p cueward-adapter-macos screenshot -- --nocapture`
`cargo test -p cueward-cli screenshot_tests -- --nocapture`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/adapter-macos/src/screenshot crates/cli/src/commands/screenshot.rs crates/cli/src/commands/screenshot_tests.rs
git commit -m "feat: add screenshot capture by window id"
```

## Chunk 5: Full Verification

### Task 5: Verify the feature end-to-end

**Files:**
- Modify only if docs drift:
  - `docs/specs/2026-04-15-screenshot-window-capture.md`
  - `docs/plans/2026-04-15-screenshot-window-capture.md`

- [ ] **Step 1: Run full automated verification**

Run:
`cargo test`
`cargo build --release`

Expected: PASS

- [ ] **Step 2: Run manual smoke checks on macOS**

Check:
- `cueward screenshot`
- `cueward screenshot windows`
- `cueward screenshot window --id <valid-id>`
- `cueward screenshot window --id <valid-id> --ocr`
- `cueward screenshot window --id <valid-id> --output /tmp/window-shot.png`

- [ ] **Step 3: Confirm acceptance**

Check:
- agent can list capturable windows
- agent can pick one `window_id` and capture it
- full-screen screenshot behavior still works

- [ ] **Step 4: Prepare implementation summary**

Call out:
- command structure
- capturable window filter rules
- `screencapture -l` path
- deferred fuzzy matching

Plan complete and saved to `docs/plans/2026-04-15-screenshot-window-capture.md`. Ready to execute?
