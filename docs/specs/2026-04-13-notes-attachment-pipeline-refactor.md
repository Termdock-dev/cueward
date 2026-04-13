# Notes Attachment Pipeline Refactor

## Goal

處理 epic #75 與 batch #76，先完成 Apple Notes 相關內部模組解偶，為後續 `#77` 到 `#80` 的 typed attachment resolver workstream 鋪路。

這一階段的最高原則：

- 外部 CLI 指令名稱、參數形狀、輸出格式保持相容
- 內部優先解偶，讓 notes 領域責任更清楚、更好維護
- 不在這一階段引入新的 attachment 類型行為變更

## Current Problems

開始重構前，notes 相關實作有三個主要問題：

1. `send.rs` 名稱像是「send」，實際卻承載一般 notes CRUD 與 notification，責任混雜。
2. `quick_notes.rs` 同時依賴共用 notes 操作與自己的 SQLite / AppleScript 特例邏輯，邊界不乾淨。
3. 舊的 `notes.rs` 已超過 500 行限制，capture、DB lookup、attachment enrichment、OCR 組裝都塞在同一檔，不利於後續擴充 `web_preview`、`binary`、`pdf`、`audio`、`map`、`drawing` 等 resolver。

## Non-Goals

這一階段不處理以下內容：

- 不新增或改變外部 CLI 介面
- 不正式導入 `AttachmentSegment.kind`
- 不新增 `web_preview`、`unresolved`、`pdf`、`audio`、`map`、`drawing` 等 attachment 類型輸出
- 不改變既有 notes image attachment 的使用者可見結果

## Target Architecture

將 notes 視為單一領域，依責任拆成下列邊界：

- `notes::capture`
  - 負責 Apple Notes capture 流程與 `Cue` 組裝入口
- `notes::db`
  - 負責 SQLite 路徑、read-only query、attachment relation / file lookup 等 DB 層細節
- `notes::attachments`
  - 負責 attachment placeholder、附件 enrichment orchestration
  - 既有 image/OCR pipeline 移入此處，作為後續各 resolver 的共同落點
- `notes::crud`
  - 負責一般 notes `create/update/delete/move`
- `notes::notify`
  - 負責 macOS notification
- `quick_notes`
  - 只保留 Quick Note 特有邏輯，例如 `list`、`find_unique`、`archive`
  - 共用 notes 操作走 `notes::crud`

## File Layout

第一階段完成後，預期至少有以下內部結構：

```text
crates/adapter-macos/src/
  notes/
    mod.rs
    capture.rs
    db.rs
    crud.rs
    notify.rs
    attachments/
      mod.rs
      image.rs
  quick_notes.rs
```

說明：

- 舊的 `notes.rs` 會由 `notes/` 模組樹取代
- `send.rs` 的責任會被 `notes::crud` 與 `notes::notify` 吸收
- CLI 層仍維持現有 `send`、`notes`、`quick-notes` 指令，只是內部 dispatch 到新的模組

## Compatibility Rules

以下行為必須保持相容：

- `cueward send --title ... --body ... --folder ... [--notify]`
- `cueward notes update|delete|move ...`
- `cueward quick-notes list|create|update|delete|archive ...`
- 既有 stderr 成功/失敗訊息
- 既有 notes capture JSON schema

這意味著 #76 的重點是重組內部模組，而不是調整 command surface。

## Testing Strategy

### Adapter Tests

- 保留並搬移既有 multiline note create 測試
- 保留並搬移 quick notes 的 HTML/title strip 與 archive guard 測試
- 保留並搬移 notes image attachment pipeline 的單元測試

### CLI Tests

- 為 `send`、`notes`、`quick-notes` 補最小 parsing test，確認外部參數形狀沒變

### Verification

- `cargo build --release`
- `cargo test`

## Follow-on Work

在 #76 完成後，後續 workstream 依序落到：

- #77 `AttachmentSegment.kind` + unresolved fallback
- #78 `web_preview` + `map`
- #79 `binary` + `pdf` / `scan`
- #80 `audio` + `drawing`

第一階段的成功標準不是「功能更多」，而是「後面每一題都能用加 resolver 的方式前進，而不是再做一次結構性拆分」。
