# Voice Memos Read

## Goal

處理 issue #94 的第一階段，先為 `cueward` 新增 macOS Voice Memos 的唯讀讀取能力。

這一批只做：

- list / read

暫不做：

- delete
- transcribe

原因：

- 已確認本機資料來源清楚，可先穩定做出 read path
- 目前尚未找到可直接重用的 transcript 欄位
- delete 需要確認 DB/state 與實體檔同步策略，風險較高

## 已確認的資料來源

Voice Memos 本機資料落點：

- 錄音檔：
  - `~/Library/Group Containers/group.com.apple.VoiceMemos.shared/Recordings/*.m4a`
- 資料庫：
  - `~/Library/Group Containers/group.com.apple.VoiceMemos.shared/Recordings/CloudRecordings.db`

目前已確認可用 table / column：

- `ZCLOUDRECORDING`
  - `ZCUSTOMLABEL`
  - `ZDATE`
  - `ZDURATION`
  - `ZPATH`
  - `ZUNIQUEID`

也就是說，第一版已足夠支撐：

- id
- title
- duration
- date
- file path

## CLI

新增 top-level `voice-memos` 子命令：

```bash
cueward voice-memos list
cueward voice-memos read --id <memo-id>
```

語意：

- `list`
  - 列出所有可讀的語音備忘錄
  - 每筆至少輸出 `id`、`title`、`duration_seconds`、`timestamp`、`path`
- `read`
  - 依 `id` 讀取單筆 memo metadata
  - 第一版不回傳 transcript

## 輸出格式

`list`：

```json
[
  {
    "id": "F45D4751-183C-4032-99F7-F1FE1F541BA2",
    "title": "2025-04-19T04:35:12Z",
    "duration_seconds": 1.5,
    "timestamp": "2025-04-19T12:35:12+08:00",
    "path": "20250419 123512-F45D4751.m4a"
  }
]
```

`read`：

```json
{
  "id": "F45D4751-183C-4032-99F7-F1FE1F541BA2",
  "title": "2025-04-19T04:35:12Z",
  "duration_seconds": 1.5,
  "timestamp": "2025-04-19T12:35:12+08:00",
  "path": "20250419 123512-F45D4751.m4a"
}
```

title fallback：

- 優先 `ZCUSTOMLABEL`
- 若空，退回 `ZPATH`
- 再不行，退回 `ZUNIQUEID`

## 架構

新增 adapter 模組：

- `crates/adapter-macos/src/voice_memos.rs`

責任：

- 讀 `CloudRecordings.db`
- 映射 `ZCLOUDRECORDING` rows
- 將 `ZDATE` Apple epoch 轉成本地時間
- 將 `ZPATH` 組回 `Recordings/<file>`

CLI 端：

- `crates/cli/src/commands/voice_memos.rs`
- `crates/cli/src/commands/mod.rs`

不把 Voice Memos 混進 `messages.rs` / `notes.rs`：

- 不同 app
- 底層是獨立 DB + media file
- repo 規則偏向每個功能領域一個模組

## 錯誤處理

明確區分：

- `voice memos db not found`
- `voice memo not found`
- `voice memo path missing`
- `voice memos decode failed`

如果 DB row 存在但實體檔已不存在：

- `list` 可選擇保留 metadata 並將 `path = null`
- 或先略過該 row

第一版建議：

- `list` 保留 metadata，但 `path` 可為 null
- 不因單筆壞資料讓整體 list 失敗

## 測試

至少包含：

- CLI parsing tests：
  - `voice-memos list`
  - `voice-memos read --id`
- adapter tests：
  - sqlite row 解析
  - title fallback
  - `ZDATE` 轉換
  - read by `id`
  - 缺 path row 的行為

## 驗收條件

- `cueward voice-memos list` 可列出至少一筆 memo
- `cueward voice-memos read --id ...` 可讀到單筆 metadata
- `id/title/duration/timestamp/path` 都有穩定輸出
- 沒有 transcript 的情況不會讓 read/list 失敗
