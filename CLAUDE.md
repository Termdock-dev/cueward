# Cueward — Claude Code 規範

## 技術棧

- Rust（Edition 2024、strict mode）
- Cargo workspace：`cueward-core`（跨平台）、`cueward-adapter-macos`（macOS）、`cueward-adapter-windows`（預留）、`cueward-cli`
- macOS 自動化：AppleScript（`osascript`）、SQLite 直讀、Vision Framework OCR
- 無外部服務依賴，全部本地執行

## 架構原則

### Adapter Pattern（最高優先）

核心邏輯在 `cueward-core`，平台實作在 `adapter-*`。禁止在 core 引用 macOS API。

### 模組拆分

- **單一檔案不超過 500 行**。超過就拆。
- **每個功能領域一個模組**：bookmarks、calendar、notes、reminders 各自獨立。
- **safari 模組正在重構**（見 #66）：新功能必須開新檔案，禁止往 `safari.rs` 裡塞。
- AI provider（gemini、chatgpt、grok）應各自獨立，不混在同一個檔案。

### DRY

- 重複兩次就抽共用函數。
- AppleScript 的共用 pattern（escape、run_capture）已在 `applescript.rs`，用它。
- JS builder 函數有共用模式（selector、click、fill），抽出來。

## 程式碼規範

- **`unwrap()` 限用**：只在測試裡。正式碼用 `?` 或明確 error handling。
- **函數短小**：超過 50 行就該拆。
- **命名一致**：`build_*_js()` 建 JS 字串、`*_extract_js()` 提取資料、`send_*_prompt()` 送 prompt。
- **pub fn 有 doc comment**：至少一行說明。
- **no emoji**：程式碼和文件禁止 emoji。

## Safari 模組規範

Safari 自動化是 cueward 最大的模組，遵守以下規則：

- **新 provider 開新檔案**：不要往 `safari.rs` 加。參考 #66 的目標結構。
- **所有 Safari 操作包 `with_safari_session()`**：確保 rate limit + file lock 生效。
- **JS builder 是純函數**：只回傳字串，可測試。
- **回傳 `<external>` 標記**：CLI 輸出外部內容時用 `print_external()` 包裝。
- **response extract 偵測 running/complete**：不要用 `data-testid` 做狀態判斷（會 false positive）。

## 測試

- 測試寫在 `mod tests` 裡（同檔案底部）。
- CLI parsing 測試：每個新子命令至少一個 `Cli::try_parse_from` 測試。
- JS builder 測試：驗證生成的 script 包含關鍵 selector 和邏輯。
- plist / AppleScript 測試：用 mock data 或 tempdir，不依賴系統狀態。
- 跑測試：`cargo test`

## 驗證

修改後必須：

```bash
cargo build --release          # 編譯
cargo test                     # 測試
cargo clippy -- -D warnings    # lint（可選但建議）
```

## 注意事項

- macOS 的 TCC 權限：Full Disk Access + Automation 權限，CLI 要有友善的錯誤提示。
- AppleScript 的 `date` 解析依賴系統 locale：用 `current date` + 逐欄設值，不要用 `date "string"`。
- AppleScript 的 `default calendar` 在中文系統不認：用 `first calendar` 或指定名稱。
- Safari 的 `data-testid` 是持久屬性，不能拿來判斷 UI 狀態。
