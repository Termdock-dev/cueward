# Shortcuts CLI Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a new `cueward shortcuts` command group that can create, inspect, mutate, and run Apple Shortcuts on macOS using a shared internal spec model and a DB-first installer.

**Architecture:** Model shortcuts as typed data in `cueward-core`, compile that model into Shortcuts plist action arrays in `cueward-adapter-macos`, and expose both incremental `add-*` mutations and declarative `apply` through `cueward-cli`. Exact-name lookup, rename, move, share sheet attachment, and input-type metadata all route through one macOS installer surface so CLI and spec flows stay consistent.

**Tech Stack:** Rust 2024, clap, serde, serde_json, serde_yaml, plist, rusqlite, uuid, Apple Shortcuts SQLite store.

---

## File Structure

### New files

- `crates/core/src/shortcuts.rs`
  - Typed `ShortcutSpec`, metadata, actions, references, and YAML/JSON serde support.
- `crates/adapter-macos/src/shortcuts/mod.rs`
  - Public entrypoints for shortcut CRUD-lite, action append, spec apply, and run.
- `crates/adapter-macos/src/shortcuts/types.rs`
  - Adapter-only row shapes, selectors, install results, and DB metadata structs.
- `crates/adapter-macos/src/shortcuts/db.rs`
  - SQLite reads/writes for `ZSHORTCUT`, `ZSHORTCUTACTIONS`, `ZCOLLECTION`, and `Z_4SHORTCUTS`.
- `crates/adapter-macos/src/shortcuts/compiler.rs`
  - Compile `ShortcutSpec` actions into plist-friendly dictionaries and binary payloads.
- `crates/adapter-macos/src/shortcuts/finder.rs`
  - Exact-name lookup and relation helpers.
- `crates/adapter-macos/src/shortcuts/actions.rs`
  - Builder functions for MVP actions (`text`, `replace-text`, `copy-to-clipboard`, `share`, `get-text`, `get-urls`, `if`, `repeat`).
- `crates/cli/src/commands/shortcuts.rs`
  - clap subcommands and dispatch for the new command group.
- `crates/cli/src/commands/shortcuts_tests.rs`
  - CLI parsing coverage for the new command group.
- `crates/adapter-macos/src/shortcuts/tests.rs`
  - Adapter tests against temp SQLite fixtures and compiler output.

### Modified files

- `crates/core/src/lib.rs`
  - Export `shortcuts` module.
- `crates/core/Cargo.toml`
  - Add `serde_yaml`.
- `crates/adapter-macos/src/lib.rs`
  - Export `shortcuts` module.
- `crates/cli/src/commands/mod.rs`
  - Register `shortcuts` module, tests, enum variant, and dispatch arm.
- `crates/cli/Cargo.toml`
  - Add `serde_yaml`.
- `crates/adapter-macos/Cargo.toml`
  - No new dependency expected if action compilation stays on `serde_json` + `plist`, but confirm and add only if needed.

### Testing strategy files

- `crates/cli/src/commands/shortcuts_tests.rs`
- `crates/adapter-macos/src/shortcuts/tests.rs`

### Manual validation targets

- Live shortcut `Clean URL Share`
- Temp shortcuts created during implementation smoke tests

---

## Chunk 1: Core Model And Spec Serialization

### Task 1: Add shortcut spec types to `cueward-core`

**Files:**
- Create: `crates/core/src/shortcuts.rs`
- Modify: `crates/core/src/lib.rs`
- Modify: `crates/core/Cargo.toml`
- Test: `crates/core/src/shortcuts.rs`

- [ ] **Step 1: Add YAML dependency for spec serialization**

Modify `crates/core/Cargo.toml`:

```toml
[dependencies]
serde_yaml = "0.9"
```

- [ ] **Step 2: Define the root spec model**

Create `crates/core/src/shortcuts.rs` with initial types:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShortcutSpec {
    pub name: String,
    #[serde(default)]
    pub surfaces: Vec<ShortcutSurface>,
    pub input: ShortcutInputPolicy,
    #[serde(default)]
    pub actions: Vec<ShortcutAction>,
}
```

- [ ] **Step 3: Define metadata enums**

Add the first metadata types:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ShortcutSurface {
    LibraryRoot,
    ShareSheet,
    Folder(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ShortcutInputPolicy {
    Any,
    Url,
    Urls,
    Text,
    Image,
    File,
}
```

- [ ] **Step 4: Define the MVP action enum**

Include only the first validated action families:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ShortcutAction {
    Text { value: String, output: Option<String> },
    GetText { from: ShortcutReference, output: Option<String> },
    GetUrls { from: ShortcutReference, output: Option<String> },
    ReplaceText {
        from: ShortcutReference,
        find: String,
        replace: String,
        regex: bool,
        ignore_case: bool,
        output: Option<String>,
    },
    CopyToClipboard { from: ShortcutReference },
    Share { from: ShortcutReference },
    IfEqualsText {
        input: ShortcutReference,
        value: String,
        then_actions: Vec<ShortcutAction>,
    },
    RepeatEach {
        input: ShortcutReference,
        body: Vec<ShortcutAction>,
    },
}
```

- [ ] **Step 5: Add a typed reference model**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum ShortcutReference {
    Output(String),
    ExtensionInput,
    RepeatItem,
    RepeatIndex,
}
```

- [ ] **Step 6: Add serialization round-trip tests**

Append tests in `crates/core/src/shortcuts.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_round_trip_preserves_clean_url_share_shape() {
        let yaml = r#"
name: Clean URL Share
surfaces:
  - library-root
  - share-sheet
input:
  type: url
actions:
  - type: get-text
    from:
      kind: extension-input
    output: input_url_text
"#;

        let spec: ShortcutSpec = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(spec.name, "Clean URL Share");
        assert_eq!(spec.surfaces.len(), 2);
        assert_eq!(spec.actions.len(), 1);
    }
}
```

- [ ] **Step 7: Export the new module**

Modify `crates/core/src/lib.rs`:

```rust
pub mod shortcuts;
pub use shortcuts::{ShortcutAction, ShortcutInputPolicy, ShortcutReference, ShortcutSpec, ShortcutSurface};
```

- [ ] **Step 8: Run focused core tests**

Run:

```bash
cargo test -p cueward-core shortcuts -- --nocapture
```

Expected: all new shortcut model tests pass.

- [ ] **Step 9: Commit**

```bash
git add crates/core/src/shortcuts.rs crates/core/src/lib.rs crates/core/Cargo.toml
git commit -m "feat: add core shortcut spec model"
```

---

## Chunk 2: macOS Shortcut Installer And Compiler

### Task 2: Add SQLite reads/writes and installer entrypoints

**Files:**
- Create: `crates/adapter-macos/src/shortcuts/mod.rs`
- Create: `crates/adapter-macos/src/shortcuts/types.rs`
- Create: `crates/adapter-macos/src/shortcuts/db.rs`
- Modify: `crates/adapter-macos/src/lib.rs`
- Test: `crates/adapter-macos/src/shortcuts/tests.rs`

- [ ] **Step 1: Define adapter-only selector and result types**

Create `crates/adapter-macos/src/shortcuts/types.rs`:

```rust
#[derive(Debug, Clone)]
pub enum ShortcutSelector {
    Id(String),
    Name(String),
}

#[derive(Debug, Clone)]
pub struct ShortcutRecord {
    pub pk: i64,
    pub name: String,
    pub workflow_id: String,
    pub action_count: i64,
}
```

- [ ] **Step 2: Add DB lookup helpers**

Create `crates/adapter-macos/src/shortcuts/db.rs` with helpers for:

- open `~/Library/Shortcuts/Shortcuts.sqlite`
- fetch shortcut by exact name
- fetch shortcut by workflow id
- fetch share-sheet relation
- fetch folder relations

Minimum function signatures:

```rust
pub fn find_shortcut(selector: &ShortcutSelector) -> Result<ShortcutRecord, MacosError>;
pub fn list_shortcuts() -> Result<Vec<ShortcutRecord>, MacosError>;
```

- [ ] **Step 3: Add transactional payload writer**

Add a function that writes:

- `ZSHORTCUTACTIONS.ZDATA`
- `ZSHORTCUT.ZACTIONCOUNT`
- `ZSHORTCUT.ZWORKFLOWSUBTITLE`
- `ZSHORTCUT.ZACTIONSDESCRIPTION`
- `ZSHORTCUT.ZINPUTCLASSESDATA`
- `ZSHORTCUT.ZHASSHORTCUTINPUTVARIABLES`

Suggested signature:

```rust
pub fn write_shortcut_payload(
    shortcut_pk: i64,
    payload: &[u8],
    action_count: usize,
    input_classes: Option<&[u8]>,
    has_shortcut_input_variables: bool,
) -> Result<(), MacosError>;
```

- [ ] **Step 4: Add relation update helpers**

Add helpers for:

- ensure root relation
- ensure share-sheet relation (`collection_pk = 2`)
- attach to named folder collection

- [ ] **Step 5: Add module exports**

Modify `crates/adapter-macos/src/lib.rs`:

```rust
pub mod shortcuts;
```

- [ ] **Step 6: Add adapter smoke tests with temp fixture**

Create `crates/adapter-macos/src/shortcuts/tests.rs` with fixture DB tests:

```rust
#[test]
fn write_shortcut_payload_updates_action_count_and_blob() {
    // build temp sqlite fixture mirroring ZSHORTCUT + ZSHORTCUTACTIONS
    // run write_shortcut_payload
    // assert updated count and blob
}
```

- [ ] **Step 7: Run focused adapter tests**

Run:

```bash
cargo test -p cueward-adapter-macos shortcuts::tests -- --nocapture
```

Expected: DB fixture tests pass.

- [ ] **Step 8: Commit**

```bash
git add crates/adapter-macos/src/lib.rs crates/adapter-macos/src/shortcuts
git commit -m "feat: add macos shortcut db installer"
```

### Task 3: Add plist action compiler for the MVP action set

**Files:**
- Create: `crates/adapter-macos/src/shortcuts/compiler.rs`
- Create: `crates/adapter-macos/src/shortcuts/actions.rs`
- Modify: `crates/adapter-macos/src/shortcuts/mod.rs`
- Test: `crates/adapter-macos/src/shortcuts/tests.rs`

- [ ] **Step 1: Add helper builders for plist variable references**

In `actions.rs`, implement focused helpers:

```rust
fn action_output_ref(output_name: &str, output_uuid: &str) -> serde_json::Value
fn extension_input_text_token() -> serde_json::Value
fn text_token_from_output(output_name: &str, output_uuid: &str) -> serde_json::Value
```

- [ ] **Step 2: Add builders for text and replace-text**

Implement:

```rust
fn build_text_action(...)
fn build_replace_text_action(...)
```

Use the already-validated keys:

- `is.workflow.actions.gettext`
- `is.workflow.actions.text.replace`
- `WFReplaceTextFind`
- `WFReplaceTextReplace`
- `WFReplaceTextRegularExpression`
- `WFReplaceTextCaseSensitive`

- [ ] **Step 3: Add builders for clipboard and share**

Implement:

```rust
fn build_setclipboard_action(...)
fn build_share_action(...)
```

- [ ] **Step 4: Add builders for get-text, get-urls, if, repeat**

Even if some are not fully validated yet, define the typed compiler surface so CLI and spec do not need redesign later.

If `get-urls` still depends on an unresolved payload shape, implement it behind an explicit error until validated:

```rust
ShortcutAction::GetUrls { .. } => {
    return Err(MacosError::Other("get-urls action not yet validated on macOS".into()));
}
```

This is better than a fake implementation.

- [ ] **Step 5: Add the full compiler entrypoint**

In `compiler.rs`:

```rust
pub fn compile_actions(spec: &ShortcutSpec) -> Result<Vec<u8>, MacosError> {
    // Vec<serde_json::Value> -> plist binary bytes
}
```

- [ ] **Step 6: Add compiler snapshot-style tests**

Add tests for:

- `text -> clipboard`
- `clean-url` subset without `get-urls`
- UUID uppercase format
- output name wiring

- [ ] **Step 7: Run focused compiler tests**

Run:

```bash
cargo test -p cueward-adapter-macos compiler -- --nocapture
```

Expected: compiler tests pass and binary plist output is generated.

- [ ] **Step 8: Commit**

```bash
git add crates/adapter-macos/src/shortcuts/compiler.rs crates/adapter-macos/src/shortcuts/actions.rs crates/adapter-macos/src/shortcuts/tests.rs
git commit -m "feat: add shortcut plist compiler"
```

---

## Chunk 3: CLI Commands And Spec Apply

### Task 4: Add clap command surface for `cueward shortcuts`

**Files:**
- Create: `crates/cli/src/commands/shortcuts.rs`
- Create: `crates/cli/src/commands/shortcuts_tests.rs`
- Modify: `crates/cli/src/commands/mod.rs`
- Modify: `crates/cli/Cargo.toml`

- [ ] **Step 1: Add YAML dependency to CLI**

Modify `crates/cli/Cargo.toml`:

```toml
[dependencies]
serde_yaml = "0.9"
```

- [ ] **Step 2: Define clap subcommands**

Create `crates/cli/src/commands/shortcuts.rs`:

```rust
#[derive(Subcommand)]
pub(crate) enum ShortcutsAction {
    Create { name: String },
    Show { #[arg(long)] id: Option<String>, #[arg(long)] name: Option<String> },
    List,
    Run { #[arg(long)] id: Option<String>, #[arg(long)] name: Option<String> },
    Rename { /* selector + new_name */ },
    Move { /* selector + folder */ },
    Surface { /* selector + surface */ },
    InputType { /* selector + input type */ },
    AddText { /* selector + value + output */ },
    AddReplaceText { /* selector + from + find + replace + flags + output */ },
    AddCopyToClipboard { /* selector + from */ },
    AddShare { /* selector + from */ },
    AddGetText { /* selector + from + output */ },
    AddGetUrls { /* selector + from + output */ },
    AddIf { /* selector + input + value */ },
    AddRepeat { /* selector + input */ },
    Apply { path: String },
    ExportSpec { /* selector */ },
    ValidateSpec { path: String },
}
```

- [ ] **Step 3: Register the command in `mod.rs`**

Modify:

- module declarations
- `pub(crate) use ...`
- `Command` enum
- `dispatch` match arm

- [ ] **Step 4: Add selector validation helper**

Follow the existing reminders pattern:

```rust
fn shortcut_selector(...) -> cueward_adapter_macos::shortcuts::ShortcutSelector
```

- [ ] **Step 5: Add parse tests**

Cover:

- `create`
- `apply`
- `add-replace-text`
- selector mutual exclusion
- missing selector failure

- [ ] **Step 6: Run CLI parsing tests**

Run:

```bash
cargo test -p cueward-cli shortcuts_tests -- --nocapture
```

Expected: all new CLI parse tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/cli/src/commands/shortcuts.rs crates/cli/src/commands/shortcuts_tests.rs crates/cli/src/commands/mod.rs crates/cli/Cargo.toml
git commit -m "feat: add shortcuts cli command surface"
```

### Task 5: Wire `create`, `apply`, `rename`, `move`, `surface`, `input-type`, and first `add-*`

**Files:**
- Modify: `crates/adapter-macos/src/shortcuts/mod.rs`
- Modify: `crates/cli/src/commands/shortcuts.rs`
- Test: `crates/adapter-macos/src/shortcuts/tests.rs`

- [ ] **Step 1: Add `create` implementation**

Use the validated internal intent flow or DB shell creation path to create a blank shortcut and return its workflow id.

- [ ] **Step 2: Add `apply` implementation**

Flow:

- parse YAML into `ShortcutSpec`
- resolve target or create new shortcut shell
- compile actions
- write DB payload
- apply surfaces and input metadata

- [ ] **Step 3: Add mutation helpers for first `add-*` commands**

Each `add-*` should:

- load existing spec shape from current shortcut or initialize append-only model
- append one typed action
- recompile full payload
- write transactionally

Start with:

- `add-text`
- `add-replace-text`
- `add-copy-to-clipboard`
- `add-share`

- [ ] **Step 4: Add `show` and `export-spec`**

Return high-level spec shape, not raw plist dump.

- [ ] **Step 5: Add manual smoke test command sequence**

Run:

```bash
cargo run -p cueward-cli -- shortcuts create "Plan Smoke"
cargo run -p cueward-cli -- shortcuts add-text --name "Plan Smoke" --value "hello" --output greeting
cargo run -p cueward-cli -- shortcuts add-copy-to-clipboard --name "Plan Smoke" --from greeting
cargo run -p cueward-cli -- shortcuts run --name "Plan Smoke"
pbpaste
```

Expected: clipboard becomes `hello`.

- [ ] **Step 6: Commit**

```bash
git add crates/adapter-macos/src/shortcuts/mod.rs crates/cli/src/commands/shortcuts.rs crates/adapter-macos/src/shortcuts/tests.rs
git commit -m "feat: wire shortcut installer and mvp actions"
```

---

## Chunk 4: Live Validation, Docs, And Handoff

### Task 6: Verify the end-to-end `Clean URL Share` path and document known gaps

**Files:**
- Modify: `docs/specs/2026-04-19-shortcuts-cli-design.md` (only if behavior changed)
- Modify: `docs/lessons.md`
- Test: live manual validation only

- [ ] **Step 1: Validate the fixed-url smoke test**

Use the already-created `Clean URL Share`-style payload or CLI-generated equivalent.

Run:

```bash
cargo run -p cueward-cli -- shortcuts create "Clean URL Share Plan Test"
# build via add-* or apply
cargo run -p cueward-cli -- shortcuts run --name "Clean URL Share Plan Test"
pbpaste
```

Expected after the fixed URL test payload:

```text
https://www.youtube.com/watch?v=abc123
```

- [ ] **Step 2: Validate share-sheet metadata persistence**

Check:

- shortcut appears in share sheet collection
- input classes restrict to `WFURLContentItem`

- [ ] **Step 3: Record the still-open limitations**

Append to `docs/lessons.md`:

- delete remains interactive-only or unsupported
- `SearchShortcutsAction` not used as primary finder
- `get-urls` builder still blocked unless a validated payload is discovered

- [ ] **Step 4: Run project verification**

Run:

```bash
cargo build --release
cargo test
```

Expected: build succeeds; tests pass.

- [ ] **Step 5: Commit**

```bash
git add docs/lessons.md docs/specs/2026-04-19-shortcuts-cli-design.md
git commit -m "docs: record shortcuts cli validation notes"
```

---

Plan complete and saved to `docs/plans/2026-04-19-shortcuts-cli.md`. Ready to execute?
