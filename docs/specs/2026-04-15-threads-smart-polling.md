# Threads Smart Polling

## Goal

處理 issue #72，將既有 smart polling state layer 補到 Threads provider，但只限 `list` / feed 掃描。

## Scope

### In scope

- `cueward safari ai --provider threads list`
- 持久化 scan state
- backoff / skip
- content pre-filter
- `fresh` / `unchanged` / `skipped` envelope 輸出

### Out of scope

- Threads `post`
- Threads `search`
- 2-strike delete detection
- 新的 shared social abstraction

## Design

Threads 沿用既有 `scan_state` helper，與 X 對齊 envelope 形狀與 state schema，但不引入單貼文 deleted 判定。因為 Threads 目前只有 feed list，空結果不代表 target 消失，直接套 2-strike delete 誤殺風險高。

`threads_extract_feed()` 會：

1. 用 canonical target URL 建 key
2. 讀 `State`
3. 先檢查 `skip_reason()`
4. 抓原始 Threads feed
5. 過濾太短、bot-like、太舊內容
6. 對過濾後結果算 fingerprint
7. 用 `record_success()` 產生 `fresh` / `unchanged`
8. 存回 state，回傳 `ScanEnvelope<Vec<SocialFeedPost>>`

## Canonical Target

- feed target: `https://www.threads.com/`

## Filtering

沿用現有 `scan_state` 規則：

- 太短內容跳過
- bot / deleted-like author 跳過
- 超過年限內容跳過

## CLI Output

`threads list` 改為輸出 envelope JSON，不再直接輸出 raw array。`data` 欄位仍為原本的 `SocialFeedPost[]`。

## Acceptance Criteria

- 第一次 feed 掃描回 `fresh`
- 相同結果重跑回 `unchanged`
- 達 backoff 條件時回 `skipped`
- pre-filter 會排掉短文、bot-like、太舊內容
- CLI 輸出 envelope JSON
