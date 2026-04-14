# Notes Web Preview Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve Apple Notes `public.url` attachments into structured `web_preview` segments without changing the existing image or unresolved behavior.

**Architecture:** Query `public.url` attachment rows directly from `ZICCLOUDSYNCINGOBJECT` using the `attachment.ZNOTE -> note.Z_PK` relationship, then merge those rows into the existing notes attachment enrichment pipeline. Populate `kind = web_preview` with a title and URL, avoid OCR, and leave map attachments for a follow-up because they appear to use the separate `ZICLOCATION` table rather than the `public.url` path.

**Tech Stack:** Rust Edition 2024, rusqlite, serde, cargo test

---

### Task 1: Capture `public.url` rows from the Notes database

**Files:**
- Modify: `crates/adapter-macos/src/notes/db.rs`

- [ ] **Step 1: Write failing DB-level tests or parsing-level tests for `public.url` rows**

Cover:
- title comes from `attachment.ZTITLE` when present
- URL comes from `attachment.ZURLSTRING`
- note linkage uses `attachment.ZNOTE = note.Z_PK`
- rows with empty URL are ignored

- [ ] **Step 2: Run the targeted notes DB tests and verify they fail**

Run:
`cargo test -p cueward-adapter-macos notes::db::tests -- --nocapture`

Expected: FAIL because web preview loading does not exist yet.

- [ ] **Step 3: Implement a minimal `public.url` loader**

Add a new read-only query path in `notes::db` that returns note-linked URL attachment rows with:
- note timestamp
- note title
- attachment title
- URL

- [ ] **Step 4: Re-run the targeted DB tests**

Run:
`cargo test -p cueward-adapter-macos notes::db::tests -- --nocapture`

Expected: PASS

### Task 2: Emit `kind = web_preview` segments in the attachment pipeline

**Files:**
- Modify: `crates/adapter-macos/src/notes/attachments/mod.rs`
- Modify: `crates/adapter-macos/src/notes/capture.rs` only if the loader/plumbing requires it

- [ ] **Step 1: Write failing attachment tests for web previews**

Cover:
- a note with a matched `public.url` attachment emits `kind = web_preview`
- the segment includes title and URL
- `has_ocr = false`
- unresolved fallback still works when no `public.url` row is available

- [ ] **Step 2: Run the targeted attachment tests and verify they fail**

Run:
`cargo test -p cueward-adapter-macos web_preview -- --nocapture`

Expected: FAIL because the notes attachment pipeline does not know about web previews yet.

- [ ] **Step 3: Implement the minimal web preview resolver**

Add a typed segment path that:
- prefers attachment title
- falls back to note title or URL when title is missing
- avoids OCR and fake file paths
- coexists with existing image and unresolved behavior

- [ ] **Step 4: Re-run the targeted attachment tests**

Run:
`cargo test -p cueward-adapter-macos web_preview -- --nocapture`

Expected: PASS

### Task 3: Full verification

**Files:**
- Modify: `docs/plans/2026-04-13-notes-web-preview.md` only if implementation drifted

- [ ] **Step 1: Run full verification**

Run:
`cargo test`
`cargo build --release`

Expected: PASS

- [ ] **Step 2: Confirm scope boundaries**

Check:
- `public.url` notes no longer collapse to unresolved when URL metadata exists
- map/location work is still deferred

- [ ] **Step 3: Prepare handoff**

Document in the eventual PR summary:
- this batch handles `public.url / web_preview`
- map/location extraction should follow using `ZICLOCATION` / `ZLOCATION`

Plan complete and saved to `docs/plans/2026-04-13-notes-web-preview.md`. Ready to execute.
