# Cueward

> **Cue**: A signal, hint, or piece of information waiting to be processed (from browsing history, notes, messages, etc.).
> **Ward**: To guard, manage, and watch over. 
> Cueward captures these scattered signals and hands them over to an Agent to determine the next actionable step.

## Vision

Modern knowledge workers suffer from information fragmentation. Insights are scattered across devices and applications—Safari bookmarks, Apple Notes drafts, iMessage links, and Calendar events. 

**Cueward** is not another note-taking app. It is a CLI-based Agent tool focused strictly on **Capture** and **Triage**. 

Its core mission is to **converge daily knowledge accumulation**. By utilizing native system APIs (e.g., macOS AppleScript, local SQLite databases, Vision Framework), Cueward silently gathers the raw materials of your day, passes them to an AI Agent for summarization and tagging, and transforms digital clutter into structured, actionable intelligence.

## Core Commands

Cueward focuses on turning captured cues into tangible next steps:

*   `cueward capture`: Proactively scans and extracts knowledge fragments from targeted applications (browser history, notes, messages) within a specific timeframe.
*   `cueward triage`: Categorizes, summarizes, and automatically tags captured cues, routing them back into designated smart folders or archives.
*   `cueward plan`: Analyzes cues for time-sensitive tasks or events, automatically scheduling them in Calendar or creating Reminders.
*   `cueward send`: Dispatches the processed knowledge or actions (e.g., compiling a daily digest note, or triggering a native GUI notification like a Dynamic Island pop-up).

## Architecture

To ensure long-term viability and potential cross-platform support (e.g., future Windows integration with Edge/Chrome and To Do), Cueward adopts a **Core Engine + Adapter Pattern** architecture:

1.  **Core Engine (Rust)**: Handles CLI parsing, defines the unified `Cue` data structure, and manages LLM (Agent) interaction logic.
2.  **Platform Adapters**:
    *   **`macos-adapter` (MVP)**: Deeply integrates with the Apple ecosystem using native capabilities (AppleScript, `History.db`, `chat.db`, Vision OCR).
    *   **`windows-adapter` (Future)**: Provides interfaces for future Windows support via native APIs.

## Technical Taste

-   **Native First**: Zero reliance on bloated third-party libraries or fragile web scrapers. If SQLite is accessible, read it. If not, use AppleScript. If the window is opaque, use native OCR.
-   **Privacy Centric**: All data extraction (including Safari history and iMessages) is performed entirely on the local machine.
-   **Elegant Feedback**: Goes beyond terminal standard output. Background tasks can trigger low-level macOS GUI components (like a borderless notch animation) for tasteful, non-intrusive visual feedback.

---

# Cueward (中文版)

> **Cue (線索、提示)**: 捕捉來自瀏覽紀錄、備忘錄、訊息等各處的待處理訊號。
> **Ward (守望、代管)**: 像管家一樣，將這些混亂的線索妥善收斂，交給 Agent 轉化為具體的下一步。

## 專案願景

現代知識工作者深受資訊碎片化所苦。靈感與資料散落在各個裝置與應用程式中——Safari 的瀏覽紀錄、Apple Notes 的草稿、iMessage 裡的連結，以及行事曆的排程。

**Cueward** 不是另一個筆記軟體，而是一個專注於 **「捕捉 (Capture)」與「收斂 (Triage)」** 的 CLI Agent 工具。

它的核心任務是：**每天幫忙收斂知識累積**。
透過呼叫本地原生 API（如 macOS 的 AppleScript、SQLite 資料庫、Vision Framework），Cueward 能在背景匯集你一整天的資訊碎片，交由 AI Agent 進行摘要、打標籤與建立待辦，讓知識不再淪為數位垃圾。

## 核心指令

Cueward 強調「將捕捉到的 Cue 轉化為行動」的實際行為：

*   `cueward capture`：主動掃描並捕捉指定時間範圍內，散落在各應用程式（瀏覽器、備忘錄、訊息）中的知識碎片與訊號。
*   `cueward triage`：對捕捉到的 Cue 進行分類、摘要，並自動打上標籤，歸檔回對應的智慧型資料夾或知識庫。
*   `cueward plan`：分析 Cue 中的待辦事項與時間點，自動在行事曆排程或新增提醒事項。
*   `cueward send`：將收斂完成的知識或行動結果發送出去（例如：建立一則統整筆記，或透過系統原生的浮動 UI 給予通知）。

## 系統架構

為了長期的跨平台發展潛力（例如未來支援 Windows 系統），Cueward 採用 **「核心引擎 + 平台適配器 (Adapter Pattern)」** 的架構設計：

1.  **Core Engine (Rust)**：負責指令解析 (CLI)、統一定義 `Cue` 資料結構，以及與 LLM (Agent) 的交互邏輯。
2.  **Platform Adapters**：
    *   **`macos-adapter` (MVP)**：深耕 Apple 生態系，利用系統原生能力（AppleScript、`History.db`、`chat.db`、原生 Vision OCR）。
    *   **`windows-adapter` (未來規劃)**：預留介面，未來可透過實作 Windows 原生 API 來支援跨平台知識收斂。

## 技術品味

-   **原生優先**：拒絕臃腫的第三方依賴與不穩定的網頁爬蟲。能讀 SQLite 就直讀，不能讀就用 AppleScript，遇到封閉介面就呼叫原生 OCR 截圖辨識。
-   **隱私安全**：所有的資料提取（包含 Safari 瀏覽紀錄、iMessage）皆嚴格在本地設備完成。
-   **優雅回饋**：不侷限於終端機的純文字輸出。Agent 在背景執行完任務後，可透過呼叫 macOS 底層 GUI（例如繪製無邊框的靈動島動畫），給予極簡且優雅的視覺回饋。
