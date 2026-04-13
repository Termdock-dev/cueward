# Smart Polling State Layer

## Goal

處理 issue #59，為 read-oriented provider 新增跨執行的 scan state / cache 層，支援：

- scan backoff
- 2-strike delete detection
- content pre-filter

第一版範圍只接：

- Reddit
- X

不在第一版：

- Threads（另開 #72）
- Safari generic read pipeline
- 持久化全文內容 cache

## 核心概念

對重複掃描的 target（feed、post、search），以：

- `provider + canonical target URL`

作為唯一 key，持久化追蹤其最近掃描狀態。

scan state 用來決定：

- 這次要不要掃
- 掃完後算不算有變化
- 遇到 not-found / parse miss 是否應標 deleted
- 哪些內容要先在 provider 層過濾掉，不丟到上層

## State 存放

沿用 `~/.cueward/state.json`，不要再發明第二個 state 檔。

在既有 `State` 結構裡新增一塊 scan 狀態，例如：

- `scan_targets: HashMap<String, ScanTargetState>`

key 格式：

- `reddit:https://old.reddit.com/r/rust.json?limit=20`
- `reddit:https://old.reddit.com/r/rust/comments/abc123/example_title.json?limit=500`
- `x:https://x.com/search?q=rust&src=typed_query&f=live`

## State 模型

每個 target 至少追蹤：

- `provider`
- `target_url`
- `last_checked_at`
- `last_changed_at`
- `last_fingerprint`
- `no_change_count`
- `consecutive_not_found_count`
- `deleted`

語意：

- `last_checked_at`
  - 最後一次實際發 request 的時間
- `last_changed_at`
  - 最後一次偵測到內容有變化的時間
- `last_fingerprint`
  - 上次成功掃描內容的摘要指紋
- `no_change_count`
  - 連續幾次掃描都沒有變化
- `consecutive_not_found_count`
  - 連續幾次遇到 not-found / 等價 deleted 訊號
- `deleted`
  - 已確定視為 deleted，後續預設不再掃

## Scan Backoff 規則

第一版採簡單規則，不做過度最佳化：

- `no_change_count == 0`
  - 正常頻率
- `no_change_count >= 2`
  - 提高最小掃描間隔
- `last_changed_at` 距今超過一段時間（例如 3 天）
  - 可直接 skip

第一版建議：

- 正常最小間隔：30 分鐘
- 2 次以上沒變：6 小時
- 3 天以上沒變：skip

這些值先做成常數，不先做使用者配置。

## 2-Strike Delete Detection

對可能代表 deleted 的結果，不要單次就宣告刪除。

第一版規則：

- 單次 404 / provider-specific not-found
  - `consecutive_not_found_count += 1`
  - 回傳 warning 狀態
- 連續 2 次
  - `deleted = true`
  - 後續預設 skip

對暫時性錯誤（網路錯、5xx、429）：

- 不增加 `consecutive_not_found_count`
- 只回傳 fetch error

## Content Pre-filter

抓回 provider 內容後、回給上層前先過濾：

- 太短內容 skip
  - 例如 `< 5` 個詞或 `< 20` 個可見字元
- bot / deleted-like author skip
  - `AutoModerator`
  - `[deleted]`
  - `[removed]`
- 太舊內容 skip
  - 超過固定天數，例如 30 天

第一版先作用在：

- Reddit comments
- X posts

不先作用在：

- Reddit post 本體
- subreddit metadata

## Fingerprint 策略

fingerprint 只需要夠穩定，不需要可逆。

第一版用：

- provider-specific filtered item 清單
- 取每個 item 的 stable fields（id / content / score / comment count / timestamp）
- 序列化成 JSON 後算 SHA-256

重點：

- 先過 pre-filter，再算 fingerprint
- 避免因為被過濾掉的 bot / deleted / 太舊內容造成雜訊

## Provider 接法

### Reddit

作用在：

- `feed`
- `post`
- `search`

canonical URL：

- 直接用 adapter 已經建好的 `old.reddit.com/*.json` URL

變化判斷：

- `feed/search`
  - 基於 post list fingerprint
- `post`
  - 基於 top-level comments fingerprint

### X

作用在：

- `x list`
- `x read`
- `x prompt`（search）

canonical target：

- feed：`https://x.com/home`
- read：正規化 post URL
- prompt/search：實際 search URL

變化判斷：

- 基於 `SocialFeedPost` 經 pre-filter 後的 fingerprint

## CLI 形狀

第一版不新增新命令。

做法是把這層 state 邏輯藏在 provider 內部，讓既有命令：

- `cueward reddit ...`
- `cueward safari ai --provider x ...`

在重複掃描時自動受益。

若需要 debug，再後續補：

- `cueward debug scan-state`

但不放在第一版。

## 架構位置

新增跨 provider 的小模組，而不是把邏輯直接塞進每個 provider：

- `crates/core/src/state.rs`
  - 擴充 state schema
- `crates/adapter-macos/src/scan_state.rs`
  - scan state policy
  - key builder
  - fingerprint helper
  - backoff decision
  - delete tracking update

provider 只做：

- 組 canonical target key
- 提供 raw items
- 定義 provider-specific pre-filter

## 錯誤處理

不要讓 scan state 層把 request error 吃掉。

規則：

- state read 失敗
  - fallback 到預設空 state
- state write 失敗
  - 回 warning / log，但不要讓 fetch 結果整個失敗
- provider fetch 失敗
  - 原樣回傳錯誤
- deleted target
  - 回傳明確狀態，而不是靜默空結果

## 測試

### 純函式測試

至少覆蓋：

- canonical key 穩定
- backoff 決策
- 2-strike delete 狀態轉移
- pre-filter 規則
- fingerprint 對無變化內容穩定
- fingerprint 對有變化內容改變

### state round-trip

- `State` 新 schema 可 load/save
- 舊版沒有 `scan_targets` 的 `state.json` 仍可正常 load

### provider 測試

- Reddit feed/search/post 使用 scan state 後仍通過既有 parser tests
- X list/search/read 需要新增 focused tests，至少鎖住 fingerprint input 與 pre-filter

## 與現有 issue 的關係

- `#59`：直接處理
- `#72`：Threads follow-up，後續把同一套 scan state 套進 Threads provider
