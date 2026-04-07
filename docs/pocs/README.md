# Proof of Concepts (PoC)

This directory contains the initial Python and Swift scripts written to validate the technical feasibility of extracting and manipulating data from macOS native APIs. 

These scripts serve as technical references for building the `cueward-adapter-macos` Rust crate.

## Included Modules

### Apple Notes (AppleScript via Python)
- `read_notes_poc.py`: Basic note extraction (Title, HTML Body).
- `read_folder_poc.py`: Listing notes within a specific folder.
- `move_note_poc.py`: Moving a note between folders.
- `auto_organize.py`: A bot that automatically routes notes to folders based on keyword matching.
- `auto_tagger.py`: A bot that appends hashtags to notes to leverage macOS Smart Folders.
- `create_test_note.py`: Creating a new note in a specific account (e.g., iCloud).
- `cleanup_tags.py`: Utility to fix HTML structure after a bad replacement attempt.
- `notes_tool.py`: A consolidated CLI interface for Notes operations.

### Productivity Ecosystem (AppleScript via Python)
- `ecosystem_eval.py`: Combined extraction of Notes, Reminders (incomplete tasks), and Calendar (today's events).

### Vision and OCR (Native Swift)
- `vision_ocr.swift`: A command-line utility that uses the macOS Vision Framework (`VNRecognizeTextRequest`) to perform local OCR on screenshots and output JSON coordinates.

### Communications (Swift via SQLite)
- `messages_tool.swift`: Reads `chat.db` to extract recent iMessages (Requires Full Disk Access/TCC).

### User Interface (Native Swift)
- `notch_island.swift`: A proof of concept for displaying a borderless, animated "Dynamic Island" notification at the top of the macOS screen (below the notch) for background task feedback.

## Note to Agents
These are raw, experimental scripts. Do not execute them blindly without understanding the required TCC permissions (especially for `messages_tool.swift`) or potential side effects (e.g., `auto_tagger.py`). Use them to understand the required `osascript` syntax, Swift bridging, or SQLite query structures when porting functionality to the core Rust application.
