# Calendar / Screenshot / Clipboard — 設計規格

**Goal:** 為 cueward 新增三個 macOS 整合功能：日曆讀寫、螢幕截圖（含可選 OCR）、剪貼簿讀寫。

**動機:** Ryugu（AI 助手）需要這三個能力來更好地輔助老闆 — 知道行程判斷何時打擾、主動看螢幕狀態、存取剪貼簿省去手動貼上。

---

## 1. Calendar

### CLI 介面

```
cueward calendar list [--from <datetime>] [--to <datetime>] [--calendar <name>]
cueward calendar today
cueward calendar create --title <TITLE> --start <DATETIME> --end <DATETIME> [--calendar <NAME>] [--notes <NOTES>] [--location <LOCATION>]
cueward calendar delete --title <TITLE> --start <DATETIME> [--calendar <NAME>]
```

- `list`：列出時間範圍內的事件。`--from` 預設 now，`--to` 預設 now + 24h。
- `today`：語法糖，等同 `list --from "today 00:00" --to "today 23:59"`。
- `create`：建立事件。`--start` / `--end` 接受 ISO 8601（`2026-04-11T14:00:00`）或自然格式（`"2026-04-11 14:00"`）。`--calendar` 預設使用系統預設日曆。
- `delete`：刪除指定標題 + 開始時間的事件。需要兩者都匹配避免誤刪。

### 輸出格式

**list / today** — JSON array to stdout：
```json
[
  {
    "title": "週會",
    "start": "2026-04-11T14:00:00+08:00",
    "end": "2026-04-11T15:00:00+08:00",
    "calendar": "Work",
    "location": "Google Meet",
    "notes": "",
    "all_day": false
  }
]
```

**create / delete** — stderr 狀態訊息，無 stdout。

### 實作

- **AppleScript** via `osascript`，跟 notes/plan 同模式。
- `tell application "Calendar"` 查詢 `every event of every calendar whose start date >= X and start date <= Y`。
- 全天事件：`all_day: true`，start/end 只取日期。
- 重複事件：AppleScript 會回傳展開後的實例，不需要特別處理 recurrence rule。
- 日期解析：CLI 層用 `chrono` 解析，轉成 AppleScript 的 `date "YYYY-MM-DD HH:MM:SS"` 格式。

### 檔案

- `crates/adapter-macos/src/calendar.rs` — 新增
- `crates/cli/src/main.rs` — 加 `Calendar` subcommand + `CalendarAction` enum

---

## 2. Screenshot

### CLI 介面

```
cueward screenshot [--ocr] [--output <PATH>]
```

- 預設截全螢幕，存到 `~/.cueward/cache/screenshots/<timestamp>.png`。
- `--ocr`：截完後自動跑 Vision OCR，回傳內容含辨識文字。
- `--output`：指定輸出路徑（覆寫預設位置）。

### 輸出格式

JSON to stdout：
```json
{
  "path": "/Users/cyh/.cueward/cache/screenshots/20260411-143022.png",
  "timestamp": "2026-04-11T14:30:22+08:00",
  "ocr_text": "（如果有 --ocr flag）辨識到的文字內容..."
}
```

### 實作

- 用 macOS 內建 `screencapture -x <path>`（`-x` 靜音）。
- 確保 `~/.cueward/cache/screenshots/` 目錄存在。
- `--ocr` 時呼叫現有的 `ocr.rs` 邏輯（已有 Vision Framework + Swift 腳本）。
- 不做 Cue 結構包裝（截圖不是 capture 流程的一部分），直接回傳獨立 JSON。

### 檔案

- `crates/adapter-macos/src/screenshot.rs` — 新增
- `crates/cli/src/main.rs` — 加 `Screenshot` subcommand

---

## 3. Clipboard

### CLI 介面

```
cueward clipboard get [--save-image <PATH>]
cueward clipboard set <TEXT>
```

- `get`：讀取當前剪貼簿。文字直接回傳；如果是圖片，存成 PNG 回傳路徑。`--save-image` 指定圖片存放路徑，預設 `~/.cueward/cache/clipboard/<timestamp>.png`。
- `set`：寫入文字到剪貼簿。

### 輸出格式

**get** — JSON to stdout：
```json
{
  "type": "text",
  "content": "剪貼簿裡的文字"
}
```
或圖片時：
```json
{
  "type": "image",
  "path": "/Users/cyh/.cueward/cache/clipboard/20260411-143022.png"
}
```

**set** — stderr 狀態訊息，無 stdout。

### 實作

- **讀取文字**：`pbpaste` 指令。
- **偵測圖片**：用 AppleScript 檢查剪貼簿類型 — `the clipboard info` 回傳 MIME type list，檢查是否含 `«class PNGf»` 或 `«class TIFF»`。
- **讀取圖片**：用小段 Swift 腳本（類似 OCR 模式）從 `NSPasteboard.general` 取圖片存成 PNG。或用 `osascript -e 'the clipboard as «class PNGf»'` 寫入檔案。
- **寫入文字**：`echo "text" | pbcopy`（透過 stdin pipe，避免 shell injection）。

### 檔案

- `crates/adapter-macos/src/clipboard.rs` — 新增
- `crates/cli/src/main.rs` — 加 `Clipboard` subcommand + `ClipboardAction` enum

---

## 共通事項

### CueSource 擴展

在 `crates/core/src/cue.rs` 加兩個 variant：
```rust
pub enum CueSource {
    Safari, Notes, Messages, Ocr,
    Calendar,   // 新增
    Screenshot, // 新增
    // Clipboard 不需要 — 剪貼簿不進 capture/triage 流程
}
```

Clipboard 不加 CueSource，因為它是即時操作，不進 inbox/triage/search 流程。
Calendar 和 Screenshot 加了以備未來需要（calendar events 可能進 capture 流程）。

### 錯誤處理

跟現有模式一致：
- 成功：exit 0，資料到 stdout，狀態到 stderr
- 失敗：exit 1，錯誤到 stderr
- 權限問題：顯示 Automation 授權指引

### 不做的事

- Calendar 重複事件的 recurrence rule 解析（AppleScript 自動展開實例）
- Screenshot 截指定 app 視窗（未來可加 `--app` flag）
- Clipboard 監聽/歷史記錄
- 這三個功能的 capture → triage → search 整合（先獨立運作，有需要再接）

### daemon 側更新

完成後需要在 Ryugu daemon 的 `tools/cueward.ts` 加對應的 action handler（calendar、screenshot、clipboard），以及更新 `buildCuewardCmd` 的 switch/case。
