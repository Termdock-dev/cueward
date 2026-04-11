# Cueward MVP 計畫

## 目標 (Goal)
建立一個專注於「捕捉 (Capture)」與「收斂 (Triage)」的 CLI Agent 工具，解決知識碎片化問題。從第一天起即採用核心與平台分離的架構。

## 核心原則 (Core Principles)
- 實用主義：解決真實存在的問題，每天自動幫忙收斂知識累積。
- 架構遠見：Cargo Workspace 隔離核心邏輯與平台 API (Adapter Pattern)。
- 原生優先：優先讀取 SQLite，其次 AppleScript，最後 OCR。
- 專業品味：極簡、專業，Rust Edition 2024。

## Phase 0：Foundation（基礎建設）
- [x] 定義 `Cue` struct（source, timestamp, content, url, metadata/tags）
- [x] 定義 `PlatformAdapter` trait（capture_browser_history, capture_notes 等）
- [x] CLI 用 clap 建立 subcommand 骨架（capture/triage）
- [x] 定義統一的 JSON 輸出格式

## Phase 1：Capture — Safari History
- [x] macOS adapter 實作 capture_browser_history（Safari History.db SQLite）
- [x] TCC 權限錯誤處理（Full Disk Access 提示）
- [x] High Watermark 狀態管理（~/.cueward/state.json）
- [x] CLI 整合：capture subcommand 呼叫 adapter 輸出 JSON

## Phase 2：Capture — Apple Notes + iMessage
- [x] 實作 capture_notes（AppleScript）
- [x] 實作 capture_messages（chat.db SQLite）
- [x] cueward capture --source all 同時收集三種來源

## Phase 3：Triage — 本地預處理
- [x] tantivy BM25 倒排索引
- [x] aho-corasick keyword auto-tagging
- [x] Triage 結果寫回索引
- [x] inbox 持久化機制（capture → inbox → triage → processed）
- [x] cueward search 指令

## Phase 4：Agent 整合（設計變更）
- [x] 決策：Cueward 不內建 LLM，保持純 tool 定位
- [x] 撰寫 Agent skill 文件（skills/cueward-agent/）
- [x] README 改寫為實用文件（安裝、用法、Agent 整合）

## Phase 5：v0.2 指令擴展
- [x] cueward send（寫入 Apple Notes + macOS 通知）
- [x] cueward plan（建立 Reminders 提醒事項）
- [x] cueward ocr（Vision Framework OCR，支援圖片 + PDF）

## Phase 7：Safari AI Provider Automation
- [x] URL-based mode navigation (bypasses DOM clicking entirely)
- [x] execCommand('insertText') for prompt input (no focus stealing)
- [x] Deep Research: full flow (prompt → plan → confirm via "ok" → poll → report)
- [x] Image generation: prompt → canvas → base64 → PNG save
- [x] Video/Music: prompt → browser-native download via `<a download>`
- [x] Conversation list/read from sidebar
- [x] Provider-based CLI structure (`--provider gemini/chatgpt`)
- [ ] ChatGPT provider implementation
- [ ] Grok provider implementation

## Phase 6+（未來）
- [ ] 靈動島 Notch UI
- [ ] Windows adapter
- [ ] Calendar event 整合（目前只有 Reminders）
