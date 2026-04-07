# Cueward MVP 計畫

## 目標 (Goal)
建立一個專注於「捕捉 (Capture)」與「收斂 (Triage)」的 CLI Agent 工具，解決知識碎片化問題。從第一天起即採用核心與平台分離的架構。

## 核心原則 (Core Principles)
- 實用主義：解決真實存在的問題，每天自動幫忙收斂知識累積。
- 架構遠見：Cargo Workspace 隔離核心邏輯與平台 API (Adapter Pattern)。
- 原生優先：優先讀取 SQLite，其次 AppleScript，最後 OCR。
- 專業品味：極簡、專業，Rust Edition 2024。

## Phase 0：Foundation（基礎建設）
- [ ] 定義 `Cue` struct（source, timestamp, content, url, metadata/tags）
- [ ] 定義 `PlatformAdapter` trait（capture_browser_history, capture_notes 等）
- [ ] CLI 用 clap 建立 subcommand 骨架（capture/triage）
- [ ] 定義統一的 JSON 輸出格式

## Phase 1：Capture — Safari History
- [ ] macOS adapter 實作 capture_browser_history（Safari History.db SQLite）
- [ ] TCC 權限錯誤處理（Full Disk Access 提示）
- [ ] High Watermark 狀態管理（~/.cueward/state.json）
- [ ] CLI 整合：capture subcommand 呼叫 adapter 輸出 JSON

## Phase 2：Capture — Apple Notes + iMessage
- [ ] 實作 capture_notes（AppleScript）
- [ ] 實作 capture_messages（chat.db SQLite）
- [ ] cueward capture --source all 同時收集三種來源

## Phase 3：Triage — 本地預處理
- [ ] tantivy BM25 倒排索引
- [ ] aho-corasick keyword auto-tagging
- [ ] Triage 結果寫回索引

## Phase 4：Triage — LLM 摘要
- [ ] LLM 整合介面（Anthropic / OpenAI API）
- [ ] Prompt 模板與結構化 JSON 輸出
- [ ] 僅對無法本地分類的 Cue 呼叫 LLM

## Phase 5+（v0.2）
- [ ] cueward plan（行事曆 / Reminders）
- [ ] cueward send（每日摘要通知）
- [ ] Vision OCR capture
- [ ] 靈動島 Notch UI
- [ ] Windows adapter
