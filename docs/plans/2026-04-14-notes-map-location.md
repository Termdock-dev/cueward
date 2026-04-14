# Notes Map Location Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve Apple Notes map attachments into structured `kind = map` segments with at least latitude/longitude and optional title/url metadata.

**Architecture:** Detect map attachments from the existing `public.url` Notes path by recognizing `maps.apple.com` URLs, then parse the `coordinate` and `name` query parameters into a dedicated map resolver. Keep this batch minimal: no screenshot extraction, no deep placemark decoding, and no separate `ZICLOCATION` path because the current local samples are not using it.

**Tech Stack:** Rust Edition 2024, rusqlite, serde, cargo test

---

### Task 1: Add the core map fields to `AttachmentSegment`

**Files:**
- Modify: `crates/core/src/cue.rs`

- [ ] **Step 1: Write failing core schema tests**

Cover:
- serializing a `kind = map` segment includes `latitude` / `longitude`
- legacy attachment JSON without those fields still decodes cleanly

- [ ] **Step 2: Run the targeted core tests and verify they fail**

Run:
`cargo test -p cueward-core cue -- --nocapture`

Expected: FAIL because `AttachmentSegment` does not yet expose map coordinates.

- [ ] **Step 3: Add minimal additive schema fields**

Implement:
- `latitude: Option<f64>`
- `longitude: Option<f64>`

- [ ] **Step 4: Re-run targeted core tests**

Run:
`cargo test -p cueward-core cue -- --nocapture`

Expected: PASS

### Task 2: Load map rows from `public.url`

**Files:**
- Modify: `crates/adapter-macos/src/notes/mod.rs`
- Modify: `crates/adapter-macos/src/notes/db.rs`

- [ ] **Step 1: Write failing DB mapping tests**

Cover:
- Apple Maps `public.url` rows map to a typed `MapAttachment`
- `coordinate=lat,lon` is parsed into `latitude` / `longitude`
- title/url are optional but preserved when available

- [ ] **Step 2: Run the targeted DB tests and verify they fail**

Run:
`cargo test -p cueward-adapter-macos notes::db::tests -- --nocapture`

Expected: FAIL because map loading does not exist yet.

- [ ] **Step 3: Implement the minimal map loader**

Use:
- `attachment.ZTYPEUTI = 'public.url'`
- `attachment.ZNOTE = note.Z_PK`
- `attachment.ZURLSTRING LIKE 'https://maps.apple.com/%'`

Return:
- note timestamp
- note title
- attachment title/url if present
- latitude/longitude

- [ ] **Step 4: Re-run targeted DB tests**

Run:
`cargo test -p cueward-adapter-macos notes::db::tests -- --nocapture`

Expected: PASS

### Task 3: Add a map resolver to the attachment pipeline

**Files:**
- Create: `crates/adapter-macos/src/notes/attachments/map.rs`
- Modify: `crates/adapter-macos/src/notes/attachments/mod.rs`
- Modify: `crates/adapter-macos/src/notes/capture.rs`

- [ ] **Step 1: Write failing map attachment tests**

Cover:
- note with placeholder + matched map row emits `kind = map`
- `has_ocr = false`
- coordinates are present
- mixed attachment indexing still works if map follows another segment

- [ ] **Step 2: Run the targeted map tests and verify they fail**

Run:
`cargo test -p cueward-adapter-macos map -- --nocapture`

Expected: FAIL because the attachment pipeline does not yet know about maps.

- [ ] **Step 3: Implement the minimal resolver**

Pipeline order:
- image
- web_preview
- map
- unresolved

Behavior:
- label placeholder with title if present, else URL if present, else `Map`
- emit typed map segment
- preserve existing fallback behavior

- [ ] **Step 4: Re-run targeted map tests**

Run:
`cargo test -p cueward-adapter-macos map -- --nocapture`

Expected: PASS

### Task 4: Full verification

**Files:**
- Modify: `docs/plans/2026-04-14-notes-map-location.md` only if implementation drifted

- [ ] **Step 1: Run full verification**

Run:
`cargo test`
`cargo build --release`

Expected: PASS

- [ ] **Step 2: Confirm minimal success criteria**

Check:
- map attachments no longer fall into unresolved when Apple Maps `public.url` data exists
- at least one field among title/url plus coordinates survives into output
- image / web_preview / unresolved behavior is unchanged

- [ ] **Step 3: Prepare handoff**

Document in the eventual PR summary:
- this batch only handles minimal map extraction
- file-backed documents still belong to #79

Plan complete and saved to `docs/plans/2026-04-14-notes-map-location.md`. Ready to execute.
