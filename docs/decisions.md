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
