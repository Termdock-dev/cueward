# Stickies CRUD

## Goal

處理 issue #93，為 `cueward` 新增 macOS Stickies（便條紙）的基本 CRUD。

需求重點：

- 先支援 list / create / update / delete
- 第一版改走 Stickies 容器資料檔，不依賴 GUI scripting
- `id` 直接使用 Stickies state 裡的 `UUID`
- 第一版只處理純文字 title / body
- 先不處理顏色、字型、視窗位置、富文字格式

## 資料來源

Stickies 本機資料落點：

- `~/Library/Containers/com.apple.Stickies/Data/Library/Stickies/.SavedStickiesState`
- `~/Library/Containers/com.apple.Stickies/Data/Library/Stickies/<UUID>.rtfd/TXT.rtf`

其中：

- `.SavedStickiesState` 保留每張便條紙的 `UUID` 與視窗/顏色等 metadata
- 文字內容存在對應的 `TXT.rtf`
- 可透過 `textutil -convert txt -stdout <TXT.rtf>` 取純文字

第一版以這兩個來源為準，不走 `System Events` UI scripting。

## CLI

新增 top-level `stickies` 子命令：

```bash
cueward stickies list
cueward stickies create --title "臨時待辦" --body "記得回覆客戶"
cueward stickies update --id <sticky-id> [--title "新標題"] [--body "更新後內容"]
cueward stickies delete --id <sticky-id>
```

語意：

- `list`
  - 列出目前所有便條紙
  - 每張輸出 `id`、`title`、`body`
  - 若 title 缺失，回傳可讀的 fallback label
- `create`
  - 需要 `--title` 與 `--body`
  - 建立新的 `UUID.rtfd` 與 state entry
- `update`
  - 需要 `--id`
  - `--title`、`--body` 至少一個
  - 只改目標 note 的內容，保留其他 state 欄位
- `delete`
  - 需要 `--id`
  - 刪除 state entry 與對應的 `UUID.rtfd`

## Identifier 策略

第一版 `id` 直接使用 `.SavedStickiesState` 的 `UUID`。

好處：

- 不需要猜 AppleScript object identity
- `list` / `update` / `delete` 可穩定 round-trip
- 和資料檔命名一致

## 輸出格式

`list`：

```json
[
  {
    "id": "DF260009-9714-421B-BB65-D2B413C55F46",
    "title": "臨時待辦",
    "body": "記得回覆客戶"
  }
]
```

`create`：

```json
{
  "created": true,
  "sticky": {
    "id": "DF260009-9714-421B-BB65-D2B413C55F46",
    "title": "臨時待辦",
    "body": "記得回覆客戶"
  }
}
```

`update`：

```json
{
  "updated": true,
  "sticky": {
    "id": "DF260009-9714-421B-BB65-D2B413C55F46",
    "title": "新標題",
    "body": "更新後內容"
  }
}
```

`delete`：

```json
{
  "deleted": true,
  "id": "DF260009-9714-421B-BB65-D2B413C55F46"
}
```

## 架構

新增 adapter 模組：

- `crates/adapter-macos/src/stickies.rs`

責任：

- 解析 `.SavedStickiesState`
- 讀取 / 改寫 `TXT.rtf`
- 建立 / 刪除 `UUID.rtfd`
- 將檔案內容映射成 Rust struct

CLI 端：

- `crates/cli/src/commands/stickies.rs`
- `crates/cli/src/commands/mod.rs`

不把 Stickies 混進 `notes.rs` 或 `quick_notes.rs`：

- 它是不同 app
- 底層資料模型不同
- repo 規則偏向每個功能領域一個模組

## 實作策略

### list / read

- 讀 `.SavedStickiesState`
- 對每個 `UUID` 找 `UUID.rtfd/TXT.rtf`
- 用 `textutil` 轉成純文字
- title fallback：
  - 若 state 中未來發現可用 title 欄位，用它
  - 否則取 body 第一個非空白行
  - 再不行就用 `Sticky <UUID-prefix>`

### create / update

- body 寫回 `TXT.rtf`
- 可透過 `textutil` 把純文字轉成 RTF
- state plist 只補 / 改必要欄位，避免洗掉其他 UI metadata

### delete

- 從 state 移除該 `UUID`
- 刪除對應 `UUID.rtfd`

## 錯誤處理

明確區分：

- `stickies state not found`
- `sticky body not found`
- `sticky not found`
- `invalid sticky id`
- `no sticky updates specified`
- `create failed`
- `update failed`
- `delete failed`
- `textutil failed`
- `plist decode failed`
- `plist write failed`

## 測試

至少包含：

- CLI parsing tests：
  - `stickies list`
  - `stickies create`
  - `stickies update`
  - `stickies delete`
- adapter tests：
  - parse `.SavedStickiesState`
  - title fallback
  - 讀 `TXT.rtf`
  - update 至少要求一個欄位
  - create/update/delete 對 tempdir 中的 state + rtfd 做 mutation

第一版不要求：

- 真機 GUI 同步驗證
- 顏色 / 視窗 frame round-trip

## 驗收條件

- `cueward stickies list` 可成功列出至少一張便條紙
- `create / update / delete` 都可用
- 無標題便條紙不會讓流程失敗
- `list` 回來的 `id` 可直接用於 `update` / `delete`
