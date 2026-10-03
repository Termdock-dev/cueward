# Architecture and Design Decisions (ADR)

This document serves as the contextual memory for any Agent working on the Cueward project. It records the core philosophy, naming conventions, and technical decisions made during the initial planning phase.

## 1. Naming: Why "Cueward"?
- **Cue (Signal/Hint)**: Represents the fragmented insights, drafts, and events scattered across a user's digital life (browser history, notes, messages).
- **Ward (Guard/Manage)**: Represents the act of converging, watching over, and managing these cues.
- **Decision**: Avoided generic names or mythological animals (like Owl, Fox, Munin) which either clash with existing open-source projects or lack professional identity. "Cueward" establishes a unique, action-oriented brand focused on "capturing signals and handing them to an Agent."

## 2. Platform Architecture: The Adapter Pattern
- **Context**: The MVP is deeply tied to the macOS ecosystem (Apple Notes, Safari, iMessage).
- **Decision**: To prevent the core logic from being contaminated by macOS-specific APIs, the project is structured as a Cargo Workspace from Day 1.
- **Structure**:
  - `cueward-core`: Domain models (e.g., the `Cue` struct) and abstract traits.
  - `cueward-adapter-macos`: Implementations using AppleScript, local SQLite, and Vision Framework.
  - `cueward-adapter-windows`: Reserved for future expansion (e.g., Edge SQLite, Windows OCR).
- **Reasoning**: This ensures cross-platform viability. A pure core allows for seamless transition to Windows or Linux without incurring massive technical debt.

## 3. Data Extraction: Native First, No Illusions
- **Context**: Initially attempted to scrape web content (e.g., Threads) directly via HTTP and use third-party browser automation (Pinchtab).
- **Decision**: Abandoned fragile web scraping and unnecessary third-party dependencies.
- **Reasoning**: Web scraping often fails due to auth walls and bot protection, leading to AI hallucinations. Instead, Cueward strictly uses native, local, and authenticated data sources:
  - Direct SQLite reads (e.g., Safari `History.db`, Messages `chat.db`).
  - System Automation (AppleScript for Notes/Reminders).
  - Native OCR (Apple Vision Framework for opaque windows).
- **Security Note**: Reading local SQLite databases often requires Full Disk Access (TCC) on macOS. The CLI must handle this gracefully and prompt the user.

## 4. Engineering Taste: Professionalism and Pragmatism
- **No Emojis**: Official documentation and code must be clean, well-spaced, and highly professional. Emojis evoke a cheap, "AI-generated toy" aesthetic and are strictly forbidden in core docs.
- **Modern Toolchain**: The Rust Edition is strictly set to `2024`. Relying on outdated muscle memory (e.g., defaulting to 2021) is considered poor engineering taste.
- **GUI Integration**: While a CLI tool, Cueward is allowed to trigger low-level, tasteful macOS GUI components (like a borderless "Notch/Dynamic Island" notification) to provide elegant, non-intrusive feedback when background triage is complete.

## 5. Information Retrieval: BM25 over Vector Databases
- **Context**: As Cueward captures thousands of knowledge fragments (Cues) daily from Safari, Notes, and iMessage, it needs a way to retrieve relevant Cues efficiently before sending them to the LLM (to avoid the "Lost in the Middle" 1M token context limits).
- **Decision**: Cueward uses a local Inverted Index with the BM25 algorithm (via the Rust `tantivy` crate) instead of trendy Vector Databases (e.g., Chroma, Qdrant) or heavy embeddings.
- **Reasoning**:
  - **Performance & Resource Cost**: Vector embeddings require running heavy neural networks locally (spinning up fans, draining battery) and consume massive disk space. A BM25 inverted index is lightning-fast to build and query, requiring only CPU and minimal disk space.
  - **Precision**: For personal knowledge management, exact keyword matching and TF-IDF relevance (BM25) often outperform semantic similarity. When searching for "Rust concurrency," users want Cues containing those exact terms, not generic articles about "fast programming languages."
  - **Incremental Indexing**: Cueward utilizes a "High Watermark" state tracking mechanism. It only fetches new data generated since the last successful `cueward capture`, writing lightweight segments to the local `~/.cueward/index` directory without blocking the CLI UX.

## 6. Pre-processing: Aho-Corasick for O(N) Tagging
- **Context**: Automatically categorizing and tagging incoming Cues based on user-defined domains (e.g., `#Rust`, `#Finance`).
- **Decision**: Use the Aho-Corasick automaton algorithm (via the `aho-corasick` or `regex` crate in Rust) for multi-pattern string search during the ingestion phase.
- **Reasoning**: Using LLMs to categorize every single scraped paragraph is incredibly slow and expensive. Aho-Corasick allows Cueward to scan millions of characters against thousands of keywords simultaneously in milliseconds (O(N) time complexity). This acts as an "Edge Compute Router," instantly tagging 80% of the content locally and reserving the expensive LLM API calls only for complex reasoning and summarization.

## 7. Same-volume relocation: no overwrite, explicit concurrency limit

For #42, the user accepted protection focused on deletion/overwriting rather than requiring atomic source-identity compare-and-swap. Same-volume rename/move use guarded exclusive root-relative Darwin rename, without payload staging, destination replacement or a copy-then-delete fallback. Observed source/parent versions narrow races but cannot condition the kernel call on the selected source inode. Post-check mismatches preserve actual observations as incomplete evidence without rollback/retry. Cross-volume removal, trash and existing-tag replacement remain separate work. See [file relocation](files-relocation.md) for the command and receipt contract.


## 8. Independent batch rename: fail-stop and verified revision advancement

For #42, batch execution accepts only explicit independent entries that pass whole-batch conflict checks. Remaining sources retain their original revisions; same-identity/path parent revisions advance only from verified child relocation results. A held canonical root, child root guards, remaining-item replanning and final aggregate checks narrow observed races without claiming CAS or isolation. One supervised worker/lifeline covers all items. Persistent aggregate evidence references full child receipts; any failure stops later submissions without replay, rollback or deletion. Plans remain observations, not execution tokens. See [batch execution](files-batch-execution.md).

## 9. Trash: verified backup and scoped quarantine before native API

For #42, deletion protection requires a retained readable independent backup before source namespace removal. Foundation native trash is path-based and does not enforce the selected-root descriptor boundary. Execution therefore uses guarded exclusive root-relative rename into a private current-user operation directory, verifies the original inode/data/metadata there, and only then calls Foundation `trashItem(at:resultingItemURL:)`. It records the actual native URL, with no invented Trash path, shell deletion or cross-volume fallback. Mismatched source replacement remains in private staging without native submission. This is not source-inode CAS or protection against another same-privilege process racing the private native path. Receipts distinguish quarantine and native outcomes; failures retain backup/staging/Trash objects without retry or rollback. Finder Put Back may refer to private staging, not the selected original path; receipt-based backup restore has a separate contract below. See [trash execution](files-trash-execution.md), including the narrowly recorded empty native `com.apple.macl` addition observed in owned-fixture acceptance.

## 10. Trash recovery: independent verified backup copy, no native removal

For #42, restoring a completed ordinary-file trash operation uses its retained verified backup rather than moving the native Trash item. The explicit original root and a fresh original-parent version guard restoration to the recorded original path without overwrite or parent creation. A new operation copies and verifies backup data/metadata in separate private same-volume staging, then uses existing guarded exclusive root-relative publication. Original Trash, backup and trash receipt remain untouched; the restored copy has a new identity. This also avoids relying on native Put Back metadata for private staging and does not require unchanged/current Trash contents. The original receipt's false recovery fields stay historical; feature discovery uses installed help. Partial/uncertain trash recovery, broader types and cross-volume restoration are excluded. Receipts record new evidence and a consistency fingerprint, not an authenticity signature, and stop without replay/rollback/cleanup on failure. See [backup restoration](files-trash-restore.md).

## 11. Package copy: explicit bounded traversal, reuse the verified tree writer

For #42, known package directories can be included only through an explicit copy-tree request flag. Default pruning and unknown-resource refusals remain intact; opt-in does not permit aliases, symlinks, unsupported metadata, cross-volume writes or larger budgets. The existing tree engine verifies every supported node privately and publishes once without overwrite, keeping the original source. A selected package and nested packages use the same node and receipt model, not a native application-installation path. Missing/false flags preserve legacy behavior and serialized shape. Completion establishes a filesystem content copy, not runnable App signatures, document semantic correctness or provider coordination. See [package copying](files-package-copy.md).

## 12. Symlink relocation: explicit leaf-object selection, never target traversal

For #42, same-volume rename/move can select only a leaf symlink through an explicit request flag, retaining default rejection elsewhere. Native link descriptors are opened relative to a no-follow parent and validated; Foundation target resource queries are not applicable. Root-relative guarded exclusive rename and held-descriptor verification are reused, with exact target-text comparison. The target can be broken or outside scope because it is never followed, selected or edited. Relative references are preserved, not repaired; their meaning can change after moving parents. This does not add source-inode CAS, copy/tree/batch/trash link support, target availability guarantees or provider coordination. Missing/false requests retain legacy behavior. See [link relocation](files-link-relocation.md).
