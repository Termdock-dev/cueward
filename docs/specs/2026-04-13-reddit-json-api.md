# Reddit JSON API

## Goal

處理 issue #58，為 `cueward` 新增獨立的 Reddit read-only provider，優先使用公開 JSON endpoint，而不是 Safari DOM automation。

第一版範圍：

- 新增頂層 CLI：`cueward reddit`
- 只做讀取，不做發文、投票、留言互動
- 底層統一走 `old.reddit.com/*.json`
- 不需要 OAuth，只帶固定 `User-Agent`
- CLI 輸出用 `print_external()` 包裝

不納入第一版：

- nested reply tree
- comment 搜尋
- Safari 整合
- generic HTTP provider framework

## CLI

新增 `reddit` 頂層命令：

```bash
cueward reddit feed <subreddit> [--limit 20]
cueward reddit post <url>
cueward reddit search <query> [--subreddit r/rust] [--limit 10]
```

語意：

- `feed`
  - 讀取 subreddit feed
  - 預設 `--limit 20`
  - `subreddit` 允許輸入 `rust` 或 `r/rust`
- `post`
  - 讀取單篇貼文
  - 回傳文章本體 + top-level comments 扁平列表
  - 支援一般 Reddit post URL；內部轉成 `old.reddit.com/.../.json`
- `search`
  - 搜尋只回貼文，不回留言
  - 若有 `--subreddit`，限制在指定 subreddit 內搜尋
  - 預設 `--limit 10`

## 輸入正規化

### subreddit

- `rust` → `rust`
- `r/rust` → `rust`
- `R/rust`、前後空白 → trim 後轉成 `rust`
- 若輸入為空、只有 `r/`、或含 `/` 深層路徑，回傳使用者錯誤

### post URL

接受常見 Reddit 貼文 URL，例如：

- `https://www.reddit.com/r/rust/comments/abc123/example_title/`
- `https://old.reddit.com/r/rust/comments/abc123/example_title/`
- `https://reddit.com/r/rust/comments/abc123/example_title`

內部統一轉成：

- `https://old.reddit.com/r/rust/comments/abc123/example_title/.json?limit=500`

若 URL 不是 Reddit 貼文，回傳使用者錯誤。

## HTTP 策略

使用 `ureq` 做同步 HTTP 請求。

原因：

- CLI 本身是同步流程，不需要引入 async runtime
- 需求是小型 read-only API client
- 比起自己直做 `TcpStream`，`ureq` 已處理 redirect、header、錯誤狀態與 response body

所有 request 都帶固定 `User-Agent`，例如：

- `cueward/0.2.x (+https://github.com/HCYT/cueward)`

endpoint：

- feed  
  `https://old.reddit.com/r/{subreddit}.json?limit={limit}`
- post  
  `https://old.reddit.com/r/{subreddit}/comments/{id}/{slug}.json?limit=500`
- search（全站）  
  `https://old.reddit.com/search.json?q={query}&limit={limit}&sort=relevance`
- search（subreddit 內）  
  `https://old.reddit.com/r/{subreddit}/search.json?q={query}&restrict_sr=on&limit={limit}&sort=relevance`

## 輸出模型

### Feed

回傳：

- `subreddit`: metadata
- `posts`: 貼文列表

建議 JSON 結構：

```json
{
  "subreddit": {
    "name": "rust",
    "display_name": "r/rust",
    "title": "The Rust Programming Language",
    "description": "A language empowering everyone...",
    "subscribers": 123456
  },
  "posts": [
    {
      "id": "abc123",
      "title": "Rust 1.90 released",
      "author": "example_user",
      "subreddit": "rust",
      "url": "https://www.rust-lang.org/",
      "permalink": "https://reddit.com/r/rust/comments/abc123/...",
      "score": 420,
      "num_comments": 37,
      "created_utc": 1760000000,
      "selftext": ""
    }
  ]
}
```

### Post

回傳：

- `post`: 單篇文章
- `comments`: top-level comments 扁平列表

建議 JSON 結構：

```json
{
  "post": {
    "id": "abc123",
    "title": "Rust 1.90 released",
    "author": "example_user",
    "subreddit": "rust",
    "url": "https://www.rust-lang.org/",
    "permalink": "https://reddit.com/r/rust/comments/abc123/...",
    "score": 420,
    "num_comments": 37,
    "created_utc": 1760000000,
    "selftext": "release notes..."
  },
  "comments": [
    {
      "id": "c1",
      "author": "commenter1",
      "body": "great release",
      "score": 12,
      "created_utc": 1760000100,
      "permalink": "https://reddit.com/r/rust/comments/abc123/.../c1"
    }
  ]
}
```

### Search

回傳：

- `query`
- `subreddit`（可為 null）
- `limit`
- `posts`

只回貼文，不回留言。

## 架構決策

這個功能不放在 `safari ai provider` 底下，原因是：

- issue #58 的核心就是「有公開 JSON API 時，優先用 API」
- Reddit 第一版完全不需要 Safari session、DOM selector、或前景/背景 tab 控制
- 硬塞進 `safari ai` 只會混淆 transport 與 provider 邊界

實作位置：

- `crates/adapter-macos/src/reddit.rs`
- `crates/adapter-macos/src/lib.rs`
- `crates/cli/src/commands/reddit.rs`
- `crates/cli/src/commands/mod.rs`

外部介面：

- `cueward_adapter_macos::reddit::*`
- `cueward reddit ...`

## 錯誤處理

第一版至少要明確區分：

- invalid subreddit
- invalid Reddit post URL
- request failed
- Reddit returned unexpected status
- invalid Reddit JSON payload
- post payload missing article section

所有錯誤都轉成清楚的 CLI 錯誤訊息，不把 raw parser panic 暴露給使用者。

## 測試

### CLI

至少覆蓋：

- `cueward reddit feed rust`
- `cueward reddit feed r/rust --limit 50`
- `cueward reddit post https://www.reddit.com/r/rust/comments/abc123/example_title/`
- `cueward reddit search "async rust"`
- `cueward reddit search "async rust" --subreddit r/rust --limit 25`

### Adapter 純函式

至少覆蓋：

- subreddit 正規化
- post URL 正規化
- feed/search URL builder
- feed JSON parser
- post + top-level comments parser
- 忽略 nested replies

### 網路測試

第一版不做真網路測試。用 fixture JSON 或內嵌字串做 deterministic parser tests。

## 與現有 issue 的關係

- `#58`：直接處理
- `#43`：這次實作會提供 Reddit read-only provider 的主要骨架，但 `#43` 是否一併關閉，要看實際 CLI 與 README 是否完整達到該 issue 的範圍
