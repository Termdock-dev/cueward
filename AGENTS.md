# Cueward — Agent 規範

所有 Agent（Codex、Claude、Gemini）在此 repo 工作時遵守以下規則。

詳細技術規範見 [CLAUDE.md](CLAUDE.md)。

## 架構

```
cueward/
  crates/
    core/               # 跨平台核心：Cue struct、traits、index、tagger
    adapter-macos/      # macOS 實作
      src/
        lib.rs          # PlatformAdapter trait impl
        applescript.rs  # AppleScript 共用工具
        safari/         # Safari：core、observe、inspect、wait、ai、social 子模組
        bookmarks.rs    # Safari 書籤 CRUD（helpers 在 bookmarks/）
        apps/           # 應用程式 discovery/open 與 AX roots/actions
        window/         # AX、snapshot/diff、背景 input、wait、Spaces
        calendar.rs     # Apple Calendar
        notes/          # Apple Notes、DB 與 attachments
        reminders.rs    # Apple Reminders
        quick_notes.rs  # Quick Notes
        clipboard.rs    # 系統剪貼簿
        screenshot/     # 螢幕與視窗截圖
        ocr.rs          # Vision Framework OCR
        plan.rs         # Apple Reminders 建立
        messages.rs     # iMessage
        error.rs        # MacosError
    adapter-windows/    # Windows 預留（僅 stub）
    cli/                # CLI 入口
      src/main.rs       # 薄入口
      src/commands/     # 各領域 CLI parsing、dispatch 與測試
  skills/cueward-agent/ # agent 使用方式的唯一維護來源
  docs/
    decisions.md        # 架構決策紀錄
    lessons.md          # 踩坑紀錄
    specs/              # 功能規格
```

## 硬規則

1. **core 不引用平台 API** — macOS 的東西只能在 adapter-macos。
2. **單一檔案不超過 500 行** — 超過就拆成子模組。
3. **Safari 已拆成子模組** — 新功能放對應子模組或新檔案，不集中塞進 `safari/mod.rs`。
4. **所有 Safari 操作包 `with_safari_session()`** — rate limit + file lock。
5. **外部內容用 `print_external()` 輸出** — prompt defense。
6. **no emoji** — 程式碼和文件禁止 emoji。
7. **Rust Edition 2024** — 不要降級。

## 開發流程

1. 新功能 → 開新分支（`feat/xxx`）。
2. 修 bug → 開新分支（`fix/xxx`）。
3. 寫測試 → CLI parsing test + adapter unit test。
4. 驗證 → `cargo build --release && cargo test`。
5. 提 PR → 等 review bot + 人工 review。

agent 可見的指令、flags、JSON、target 或完成判定變更，同一 PR 更新 [使用 skill](skills/cueward-agent/SKILL.md) 的對應 reference 與 README／docs。純內部調整不必改 skill；尚未合併或實機未驗證的能力須標明狀態。安裝與同步見 [agent-skill.md](docs/agent-skill.md)。

## Commit 規範

```
feat: 新功能
fix: 修 bug
refactor: 重構（不改行為）
style: 格式化（rustfmt）
docs: 文件
test: 測試
chore: 雜務
```

## 已知限制

- CLI 與 Safari 已模組化；檔案位置以當前目錄為準。
- 原生 background input 的 app acceptance、完整輸入隔離與實機限制見 [computer-use-progress.md](docs/computer-use-progress.md)。
- AppleScript date 解析依賴 locale，已修（用 current date 逐欄設值）。
- Safari `data-testid` 是持久屬性，不能用來判斷 UI 狀態。

## 參考

- [decisions.md](docs/decisions.md) — 架構決策
- [lessons.md](docs/lessons.md) — 踩坑紀錄
- [skills/cueward-agent/SKILL.md](skills/cueward-agent/SKILL.md) — agent 使用入口
