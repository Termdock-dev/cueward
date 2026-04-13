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
        safari.rs       # Safari 自動化（正在重構 #66，勿新增）
        bookmarks.rs    # Safari 書籤 CRUD
        calendar.rs     # Apple Calendar
        notes.rs        # Apple Notes
        reminders.rs    # Apple Reminders
        quick_notes.rs  # Quick Notes
        clipboard.rs    # 系統剪貼簿
        screenshot.rs   # 螢幕截圖
        ocr.rs          # Vision Framework OCR
        send.rs         # Apple Notes 建立
        plan.rs         # Apple Reminders 建立
        messages.rs     # iMessage
        error.rs        # MacosError
    adapter-windows/    # Windows 預留（僅 stub）
    cli/                # CLI 入口
      src/main.rs       # 指令 dispatch（正在重構 #66）
  docs/
    decisions.md        # 架構決策紀錄
    lessons.md          # 踩坑紀錄
    specs/              # 功能規格
```

## 硬規則

1. **core 不引用平台 API** — macOS 的東西只能在 adapter-macos。
2. **單一檔案不超過 500 行** — 超過就拆成子模組。
3. **safari.rs 正在重構（#66）** — 新功能必須開新檔案，禁止往裡面加 code。
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

- `safari.rs` 3,579 行，拆分計畫在 #66。
- `main.rs` 2,369 行，Phase 4 處理。
- AppleScript date 解析依賴 locale，已修（用 current date 逐欄設值）。
- Safari `data-testid` 是持久屬性，不能用來判斷 UI 狀態。

## 參考

- [decisions.md](docs/decisions.md) — 架構決策
- [lessons.md](docs/lessons.md) — 踩坑紀錄
- [#66](https://github.com/HCYT/cueward/issues/66) — safari.rs 重構計畫
