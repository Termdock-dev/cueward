# Stickies Advanced Display Controls Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Stickies display selection, geometry control, and six preset color controls on macOS using `.SavedStickiesState`.

**Architecture:** Keep the feature inside `adapter-macos` and continue using file-backed Stickies metadata. Split the existing `stickies` adapter into focused submodules so geometry parsing, color mapping, and display coordinate resolution stay isolated and testable. Treat `--display` as a coordinate-mapping helper, not as a native plist field.

**Tech Stack:** Rust 2024, `clap`, plist parsing, file I/O, `serde`/`serde_json`, macOS AppKit screen discovery, `cargo test`

---

## Chunk 1: Refactor Stickies Module Boundaries

### Task 1: Split the existing `stickies` adapter into focused modules

**Files:**
- Move: `crates/adapter-macos/src/stickies.rs` -> `crates/adapter-macos/src/stickies/mod.rs`
- Create: `crates/adapter-macos/src/stickies/state.rs`
- Create: `crates/adapter-macos/src/stickies/geometry.rs`
- Create: `crates/adapter-macos/src/stickies/color.rs`
- Create: `crates/adapter-macos/src/stickies/display.rs`
- Modify: `crates/adapter-macos/src/stickies/tests.rs`

- [ ] **Step 1: Write failing adapter tests for geometry and color helpers**

Cover:
- frame parse / format round-trip
- expanded size parse / format round-trip
- preset color mapping emits four dictionaries

- [ ] **Step 2: Run targeted tests and verify they fail**

Run:
`cargo test -p cueward-adapter-macos stickies -- --nocapture`

Expected: FAIL because helper modules and typed parsers do not exist yet.

- [ ] **Step 3: Move shared file-backed CRUD logic into `mod.rs` / `state.rs`**

Keep:
- existing `.SavedStickiesState` parsing
- existing `TXT.rtf` body read/write
- existing UUID-based CRUD flow

Add:
- typed `StickyFrame`
- typed `StickySize`
- `StickyColorPreset`
- `StickyColorScheme`

- [ ] **Step 4: Re-run targeted tests and verify they pass**

Run:
`cargo test -p cueward-adapter-macos stickies -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/adapter-macos/src/stickies
git commit -m "refactor: split stickies geometry and color helpers"
```

## Chunk 2: Extend CLI Contract

### Task 2: Add advanced Stickies arguments to CLI parsing

**Files:**
- Modify: `crates/cli/src/commands/stickies.rs`
- Modify: `crates/cli/src/commands/stickies_tests.rs`

- [ ] **Step 1: Write failing CLI parse tests**

Cover:
- `stickies create --display 2 --color blue --x 40 --y 80 --width 420 --height 260`
- `stickies update --id ... --display 3 --color gray --x 10 --y 20 --width 360 --height 220`
- reject `--x` without `--y`
- reject `--width` without `--height`

- [ ] **Step 2: Run targeted CLI tests and verify they fail**

Run:
`cargo test -p cueward-cli cli_parses_stickies -- --nocapture`

Expected: FAIL because the new arguments are not defined yet.

- [ ] **Step 3: Add the new fields to `StickiesAction::{Create, Update}`**

Add:
- `display: Option<u32>`
- `color: Option<String>`
- `x: Option<i32>`
- `y: Option<i32>`
- `width: Option<i32>`
- `height: Option<i32>`

- [ ] **Step 4: Validate pair arguments at CLI layer or dispatch boundary**

Rules:
- `x` / `y` must appear together
- `width` / `height` must appear together

- [ ] **Step 5: Re-run targeted CLI tests and verify they pass**

Run:
`cargo test -p cueward-cli cli_parses_stickies -- --nocapture`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/cli/src/commands/stickies.rs crates/cli/src/commands/stickies_tests.rs
git commit -m "test: define stickies advanced control cli"
```

## Chunk 3: Implement Geometry And Display Mapping

### Task 3: Support `display`, `x`, `y`, `width`, and `height`

**Files:**
- Modify: `crates/adapter-macos/src/stickies/mod.rs`
- Modify: `crates/adapter-macos/src/stickies/geometry.rs`
- Modify: `crates/adapter-macos/src/stickies/display.rs`
- Modify: `crates/adapter-macos/src/stickies/tests.rs`
- Modify: `crates/cli/src/commands/stickies.rs`

- [ ] **Step 1: Write failing adapter tests for geometry application**

Cover:
- create with explicit width/height writes both `Frame` and `ExpandedSize`
- update with explicit width/height rewrites both fields
- create with `display + x/y` resolves to global frame
- create without explicit geometry still cascades on target display

- [ ] **Step 2: Run targeted tests and verify they fail**

Run:
`cargo test -p cueward-adapter-macos stickies -- --nocapture`

Expected: FAIL because geometry mutation and screen mapping do not exist yet.

- [ ] **Step 3: Implement typed geometry mutation**

Add:
- helper to merge create/update options with existing frame
- helper to sync `Frame` and `ExpandedSize`
- helper to cascade new notes on a per-display basis

- [ ] **Step 4: Implement display lookup**

Add:
- macOS screen discovery returning 1-based ordered screens
- `display index -> target frame` resolution
- mapping from display-local `x/y` to global coordinates

- [ ] **Step 5: Wire geometry options into create/update dispatch**

Create:
- adapter option structs instead of expanding function signatures indefinitely

- [ ] **Step 6: Re-run targeted tests and verify they pass**

Run:
`cargo test -p cueward-adapter-macos stickies -- --nocapture`
`cargo test -p cueward-cli cli_parses_stickies -- --nocapture`

Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/adapter-macos/src/stickies crates/cli/src/commands/stickies.rs
git commit -m "feat: add stickies geometry and display controls"
```

## Chunk 4: Implement Preset Color Controls

### Task 4: Support six preset Stickies colors

**Files:**
- Modify: `crates/adapter-macos/src/stickies/color.rs`
- Modify: `crates/adapter-macos/src/stickies/mod.rs`
- Modify: `crates/adapter-macos/src/stickies/tests.rs`
- Modify: `crates/cli/src/commands/stickies.rs`

- [ ] **Step 1: Write failing color tests**

Cover:
- each preset maps to four dictionaries
- create with `--color` writes all four color fields
- update with `--color` rewrites all four color fields
- invalid color name fails clearly

- [ ] **Step 2: Run targeted tests and verify they fail**

Run:
`cargo test -p cueward-adapter-macos stickies -- --nocapture`

Expected: FAIL because preset mapping is incomplete or not wired to mutations.

- [ ] **Step 3: Implement `StickyColorPreset` mapping**

Include:
- `yellow`
- `blue`
- `green`
- `pink`
- `purple`
- `gray`

Rule:
- write all four color dictionaries together

- [ ] **Step 4: Wire `--color` into create/update**

At dispatch boundary:
- parse incoming string into `StickyColorPreset`
- return explicit error on unsupported color

- [ ] **Step 5: Re-run targeted tests and verify they pass**

Run:
`cargo test -p cueward-adapter-macos stickies -- --nocapture`
`cargo test -p cueward-cli cli_parses_stickies -- --nocapture`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/adapter-macos/src/stickies crates/cli/src/commands/stickies.rs
git commit -m "feat: add stickies preset color controls"
```

## Chunk 5: Full Verification

### Task 5: Verify the feature end-to-end

**Files:**
- Modify only if docs drift:
  - `docs/specs/2026-04-15-stickies-display-color-geometry.md`
  - `docs/plans/2026-04-15-stickies-display-color-geometry.md`

- [ ] **Step 1: Run full automated verification**

Run:
`cargo test`
`cargo build --release`

Expected: PASS

- [ ] **Step 2: Run manual smoke checks on macOS**

Check:
- create on main display
- create on external display with `--display`
- update color through all six presets
- update geometry through `x/y/width/height`
- create multiple notes on the same display without exact overlap

- [ ] **Step 3: Confirm issue #98 acceptance**

Check:
- create can target a display
- at least one color is stable
- full position / size control exists
- notes do not always stack at the same point

- [ ] **Step 4: Prepare implementation summary**

Call out:
- display implemented via global frame mapping
- geometry synchronized through `Frame` + `ExpandedSize`
- preset colors implemented via four-dictionary color schemes
- custom color is technically possible but still deferred

Plan complete and saved to `docs/plans/2026-04-15-stickies-display-color-geometry.md`. Ready to execute?
