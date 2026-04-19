# Cueward Shortcuts CLI Design

Date: 2026-04-19
Status: Proposed
Scope: macOS only

## Summary

Add a new `cueward shortcuts` command group that lets users create, inspect, mutate, and run Apple Shortcuts from the terminal.

The user-facing model should support both:

- incremental CLI editing via `add-*` subcommands
- declarative installation via `apply <spec-file>`

Both entrypoints compile into the same internal `ShortcutSpec` model. The first implementation should use a DB-first installer against `~/Library/Shortcuts/Shortcuts.sqlite`, with finder and selected mutation helpers using behaviors already validated on this machine.

## Goals

- Provide a stable Cueward CLI for building and maintaining shortcuts.
- Support creating executable shortcuts, not just empty shells.
- Support a declarative spec format that can be committed, reviewed, and replayed.
- Support enough actions to build practical automation in the first release.
- Keep macOS-specific implementation isolated in `adapter-macos`.

## Non-Goals

- Windows support.
- Full Apple-compatible public API abstraction.
- Arbitrary action reordering or insert-at-index in the MVP.
- Icon and color editing in the MVP.
- Import/export of signed `.shortcut` files in the MVP.
- Reliable delete of existing shortcuts in the MVP.
- Generic third-party App Intent authoring in the MVP.

## Current Findings

The design is based on direct validation performed on macOS 15.7.2.

Validated working paths:

- create shortcut shell via `com.apple.shortcuts.CreateWorkflowAction`
- inject executable action payloads by writing `ZSHORTCUTACTIONS.ZDATA`
- run created shortcut successfully after payload injection
- rename shortcut via `com.apple.shortcuts.RenameShortcutAction`
- move shortcut to folder via `com.apple.shortcuts.MoveShortcutToFolderAction`
- find existing shortcut by name via:
  - `getmyworkflows`
  - `repeat.each`
  - `properties.workflow(Name)`
  - conditional compare
  - action on `Repeat Item`

Validated weak or unreliable paths:

- Apple `shortcuts` CLI has no CRUD surface
- `SearchShortcutsAction` is not reliable enough to use as primary lookup
- `DeleteWorkflowAction` appears to require interactive confirmation or unresolved entity formatting

## Architecture

### Module Split

`crates/core`

- add pure data types for shortcut specs and action descriptions
- no SQLite, AppleScript, or macOS APIs

`crates/adapter-macos`

- add `shortcuts.rs` and submodules
- own all macOS-specific logic:
  - live library lookup
  - DB writes
  - plist encoding
  - action builders
  - folder/surface/input metadata updates
  - validated helper intent flows when useful

`crates/cli`

- add `commands/shortcuts.rs`
- parse clap arguments only
- convert CLI input into `ShortcutMutation` or `ShortcutSpec`
- dispatch into adapter layer

### Internal Model

Suggested core types:

- `ShortcutSpec`
- `ShortcutMetadata`
- `ShortcutInputPolicy`
- `ShortcutSurface`
- `ShortcutAction`
- `ShortcutActionRef`
- `ShortcutMutation`

Suggested shape:

```rust
pub struct ShortcutSpec {
    pub name: String,
    pub metadata: ShortcutMetadata,
    pub actions: Vec<ShortcutAction>,
}

pub struct ShortcutMetadata {
    pub surfaces: Vec<ShortcutSurface>,
    pub input: ShortcutInputPolicy,
}

pub enum ShortcutSurface {
    LibraryRoot,
    ShareSheet,
    Folder(String),
}

pub enum ShortcutInputPolicy {
    Any,
    Url,
    Text,
    Image,
    File,
    Urls,
}
```

`ShortcutAction` should be a typed enum, not a bag of raw plist fragments. The adapter compiles typed actions into plist dictionaries.

### Data Flow

1. User invokes `cueward shortcuts ...`
2. CLI resolves into:
   - `ShortcutMutation`, or
   - `ShortcutSpec`
3. macOS adapter resolves target shortcut id if needed
4. existing DB state is loaded
5. adapter applies mutation onto an in-memory `ShortcutSpec`
6. action compiler emits binary plist array
7. adapter writes transactional DB updates
8. adapter performs optional post-steps:
   - rename
   - move
   - run
9. adapter verifies expected DB state

## CLI Surface

Top-level:

```bash
cueward shortcuts <subcommand>
```

MVP commands:

- `create`
- `show`
- `list`
- `run`
- `rename`
- `move`
- `surface`
- `input-type`
- `add-text`
- `add-replace-text`
- `add-copy-to-clipboard`
- `add-share`
- `add-get-text`
- `add-get-urls`
- `add-if`
- `add-repeat`
- `apply`
- `export-spec`
- `validate-spec`

Target selection rules:

- every mutation command accepts exactly one of:
  - `--id <workflow-id>`
  - `--name <shortcut-name>`
- no fuzzy matching
- ambiguous names return error

Example flow:

```bash
cueward shortcuts create "Clean URL Share"
cueward shortcuts surface --name "Clean URL Share" share-sheet
cueward shortcuts input-type --name "Clean URL Share" url
cueward shortcuts add-get-text --name "Clean URL Share" --from extension-input --output input_url_text
cueward shortcuts add-replace-text --name "Clean URL Share" --from input_url_text --find '...' --replace '$1' --regex --ignore-case --output tracking_removed
cueward shortcuts add-copy-to-clipboard --name "Clean URL Share" --from tracking_removed
cueward shortcuts add-share --name "Clean URL Share" --from tracking_removed
```

Spec install:

```bash
cueward shortcuts apply clean-url-share.yaml
```

## Spec Format

Use YAML first.

Reasons:

- human-readable
- easy to diff in git
- easier than JSON for multiline regex and nested action config

Example:

```yaml
name: Clean URL Share
surfaces:
  - library-root
  - share-sheet
input:
  type: url
actions:
  - type: get-text
    from: extension-input
    output: input_url_text
  - type: replace-text
    from: input_url_text
    output: tracking_removed
    regex: true
    ignore_case: true
    find: '([?&])(si|utm_[^=]*|fbclid)=[^&#]*'
    replace: '$1'
  - type: copy-to-clipboard
    from: tracking_removed
  - type: share
    from: tracking_removed
```

`add-*` commands and `apply` must compile through the same internal model.

## Installer Design

### Shortcut Resolution

Do not use `SearchShortcutsAction` as the primary finder.

Use a deterministic loop finder based on already-validated behavior:

- `getmyworkflows`
- `repeat.each`
- `properties.workflow(Name)`
- compare against target
- when matched, operate on `Repeat Item`

The implementation can avoid running helper shortcuts at runtime by reading DB tables directly for simple exact-name lookups, but the validated loop behavior is the semantic source of truth.

### DB Writes

The installer is DB-first in the MVP.

Tables already confirmed relevant:

- `ZSHORTCUT`
- `ZSHORTCUTACTIONS`
- `ZCOLLECTION`
- `Z_4SHORTCUTS`

Writes required:

- `ZSHORTCUTACTIONS.ZDATA`
- `ZSHORTCUT.ZACTIONCOUNT`
- `ZSHORTCUT.ZACTIONSDESCRIPTION`
- `ZSHORTCUT.ZWORKFLOWSUBTITLE`
- `ZSHORTCUT.ZINPUTCLASSESDATA`
- `ZSHORTCUT.ZHASSHORTCUTINPUTVARIABLES`
- `Z_4SHORTCUTS` rows for share sheet and folder membership

All DB writes must be wrapped in a transaction.

### Action Compilation

Each action builder returns plist-friendly values only.

Examples for first wave:

- `get-text`
- `replace-text`
- `copy-to-clipboard`
- `share`
- `get-urls`
- `if`
- `repeat`

Variable wiring rules:

- use uppercase UUIDs
- use explicit `CustomOutputName` whenever downstream references depend on stable names
- treat `extension-input` as a first-class source in the compiler

## Error Handling

Return explicit errors for:

- shortcut not found
- multiple shortcuts matched
- unsupported action configuration
- unsupported macOS Shortcuts schema
- invalid spec
- DB write failure
- failed verification after write

Interactive-only paths must be marked clearly.

For MVP:

- `delete` should be reported as unsupported or interactive-only
- do not silently claim success on flows known to hang or no-op

## Testing Strategy

### 1. Core Tests

Add unit tests for:

- `ShortcutSpec` parsing
- action builder output
- variable reference wiring
- YAML spec validation

### 2. CLI Tests

Every command gets `Cli::try_parse_from` coverage.

At minimum:

- target selection
- required flags
- mutually exclusive `--id` / `--name`
- representative `add-*` and `apply` examples

### 3. Adapter Tests

Use temp SQLite fixtures.

Test:

- create shell metadata rows
- update action payload rows
- rename metadata update path
- folder/share sheet relation writes
- input class persistence

Do not write tests against the live user library.

### 4. Manual Verification

Keep a short manual checklist:

- create blank shortcut
- inject executable action payload
- run updated shortcut successfully
- rename existing shortcut
- move shortcut to folder
- install `Clean URL Share`
- run fixed-url smoke test

## MVP Scope

Include:

- `create`
- `show`
- `list`
- `run`
- `rename`
- `move`
- `surface`
- `input-type`
- `apply`
- first batch of `add-*`

Exclude:

- delete existing shortcuts
- arbitrary action insertion or reordering
- icon/color editing
- `.shortcut` export/import
- generic third-party intent authoring

## Recommended Implementation Order

1. core shortcut spec types
2. CLI command shell
3. adapter DB reader/writer helpers
4. action compiler for first batch
5. `create` + `apply`
6. `rename` + `move` + `surface` + `input-type`
7. `show` + `export-spec`
8. docs and smoke tests

## Risks

- Apple may change Shortcuts DB schema across macOS releases.
- Share sheet membership semantics may shift across versions.
- `DeleteWorkflowAction` may remain interactive-only.
- Some action output names are not obvious without real-world sampling.

The design mitigates this by:

- isolating all macOS behavior in one adapter module
- using typed action builders
- preferring explicit output names
- verifying DB state after every write
