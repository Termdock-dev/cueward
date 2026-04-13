# Safari Bookmarks CRUD

## Goal

處理 issue #51，為 `cueward` 新增 Safari 書籤的讀取、新增、刪除、搜尋能力。

需求重點：

- 產品入口仍在 `cueward safari ...` 之下
- 支援任意深度的資料夾路徑，例如 `Work/AI Tools`
- 以資料夾內的 `title + url` 作為書籤唯一指紋
- 第一期只處理既有資料夾內的書籤，不自動建立新資料夾

## CLI

新增 `safari bookmarks` 子命令：

```bash
cueward safari bookmarks list [--profile "Ryugu"] [--folder "Work/AI Tools"]
cueward safari bookmarks search "claude" [--profile "Ryugu"] [--folder "Work/AI Tools"]
cueward safari bookmarks add --title "Claude" --url "https://claude.ai" [--profile "Ryugu"] [--folder "Work/AI Tools"]
cueward safari bookmarks delete --title "Claude" --url "https://claude.ai" [--profile "Ryugu"] [--folder "Work/AI Tools"]
```

語意：

- `list`
  - 未指定 `--folder` 時，列出 bookmarks root 的直接 children
  - 若指定 `--profile`，則 scope 到該 profile root folder
  - 指定 `--folder` 時，列出目標資料夾的直接 children
- `search`
  - 從 root 或指定 folder 起點遞迴搜尋
  - 先做大小寫不敏感的 `title/url contains query`
- `add`
  - 需要 `--title` 與 `--url`
  - 若同資料夾內已存在完全相同的 `title + url`，回傳 duplicate error
  - 同名不同網址允許存在
- `delete`
  - 需要 `--title`、`--url`
  - 以 `folder + title + url` 精確刪除

## Folder Path 規則

- `--profile Ryugu --folder "Work/AI Tools"` 代表沿著 `Ryugu -> Work -> AI Tools` 往下走
- 若只有 `--profile Ryugu`，等同操作 `Ryugu` root folder
- `--folder "Work/AI Tools"` 代表沿著 `Work -> AI Tools` 往下走
- 路徑分隔符固定為 `/`
- 第一期不支援 folder title 本身包含 `/`
- 空 segment 不允許，例如 `Work//AI`
- 任一路徑不存在，或對應節點不是資料夾，都回傳 `invalid folder path`
- 第一期不自動建立缺失資料夾

Safari 書籤樹的解析邏輯採遞迴走訪 `Children`，可支援任意深度。

## 架構決策

產品面維持在 `cueward safari ...` 底下，但 adapter 實作依照 repo 規則放在 `bookmarks.rs`，避免繼續惡化正在重構中的 `safari.rs`。

調整為：

- `crates/adapter-macos/src/bookmarks.rs`
- `crates/adapter-macos/src/lib.rs`

外部呼叫面維持：

- CLI 仍然是 `cueward safari bookmarks ...`
- `cueward-adapter-macos` 額外暴露 `pub mod bookmarks;`

`bookmarks.rs` 負責：

- `Bookmarks.plist` 讀取與寫回
- plist tree 到 Rust 結構的映射
- folder path 走訪
- `list/search/add/delete` 商業規則

不把這次功能塞回既有 `safari.rs` 的原因：

- `AGENTS.md` / `CLAUDE.md` 明確規定 `safari.rs` 正在重構，禁止新增功能
- repo 預期的檔案位置就是 `crates/adapter-macos/src/bookmarks.rs`
- 書籤 CRUD 是本機資料檔操作，不應和 DOM automation 混在同一檔

這一版只做到「不惡化」：

- 不在 #51 順手處理 #66 的 Safari 模組重構
- 書籤 CRUD 仍保持獨立模組，但會接到共享的 Safari session guard

## 資料模型

內部至少區分兩層：

1. plist node 結構
2. 書籤操作用的 domain model

domain model 應能表達：

- folder title
- bookmark title
- bookmark url
- folder path
- children

第一期不追求完整暴露 Safari plist 的所有欄位給 CLI，但寫回時必須保留未修改節點的原始 plist 欄位。

## 讀寫策略

`Bookmarks.plist` 路徑：

- `~/Library/Safari/Bookmarks.plist`

策略：

- `list/search` 時可解析成較小的 domain model
- `add/delete` 時必須直接在 raw plist tree 上做 lossless mutation
- 成功後一次性寫回原 plist，保留未修改節點的原始欄位

不採 shell 為中心的 `plutil` 字串拼接流程，原因：

- typed parse/write 比較穩
- 較容易做 deterministic unit tests
- 可以把 plist 結構與商業規則分層
- raw plist write path 可避免特殊節點 metadata 被意外洗掉

## Safari Session Guard

所有 `safari bookmarks` 子命令都必須包在共享的 `with_safari_session()` 後面。

原因：

- 和其他 `cueward safari ...` 指令共用同一把 lock file
- 避免 `bookmarks add/delete` 與其他 Safari 指令競態寫入
- 符合 repo 的「所有 Safari 操作包 `with_safari_session()`」硬規則

## 唯一性與衝突

書籤唯一指紋定義為同資料夾內的 `title + url`。

規則：

- `add` 遇到相同 `title + url`：拒絕新增
- `same title + different url`：允許
- `delete`：必須同時匹配 `title + url`
- 若底層資料異常，出現多筆完全相同的 `title + url`，回傳 `bookmark data conflict`，不默默刪多筆

## 輸出格式

維持 cueward 現有風格，輸出結構化 JSON；CLI 層對外部書籤資料使用 `print_external()` 包裝。

`list`：

```json
{
  "folder_path": "Work/AI Tools",
  "items": [
    {
      "kind": "bookmark",
      "title": "Claude",
      "url": "https://claude.ai",
      "folder_path": "Work/AI Tools"
    },
    {
      "kind": "folder",
      "title": "Docs",
      "folder_path": "Work/AI Tools/Docs"
    }
  ]
}
```

`search`：

```json
{
  "query": "claude",
  "items": [
    {
      "title": "Claude",
      "url": "https://claude.ai",
      "folder_path": "Work/AI Tools"
    }
  ]
}
```

`add`：

```json
{
  "created": true,
  "bookmark": {
    "title": "Claude",
    "url": "https://claude.ai",
    "folder_path": "Work/AI Tools"
  }
}
```

`delete`：

```json
{
  "deleted": true,
  "bookmark": {
    "title": "Claude",
    "url": "https://claude.ai",
    "folder_path": "Work/AI Tools"
  }
}
```

## 錯誤處理

明確區分以下錯誤：

- `bookmarks plist not found`
- `invalid folder path`
- `duplicate bookmark`
- `bookmark not found`
- `bookmark data conflict`
- `plist decode failed`
- `plist write failed`

CLI 行為維持既有模式：

- 成功：exit 0，JSON 到 stdout；若內容來自 bookmark title/url，使用 `print_external()`
- 失敗：exit 1，錯誤到 stderr

## 測試策略

### 1. CLI parse tests

放在 `crates/cli/src/main.rs` 現有測試區塊，覆蓋：

- `cueward safari bookmarks list`
- `cueward safari bookmarks list --folder "Work/AI Tools"`
- `cueward safari bookmarks search "claude" --folder "Work/AI Tools"`
- `cueward safari bookmarks add --title ... --url ... --folder ...`
- `cueward safari bookmarks delete --title ... --url ... --folder ...`

### 2. 純函式測試

放在 `crates/adapter-macos/src/bookmarks.rs`，覆蓋：

- folder path 解析
- 遞迴走訪 children
- `title + url` 指紋比對
- duplicate/conflict 判斷
- search contains query 邏輯

### 3. plist round-trip tests

用 fixture plist 驗證：

- `list` 讀出正確 folder children
- `search` 可跨層命中
- `add` 成功插入新 bookmark
- `add` 對完全相同 `title + url` 報 duplicate
- `delete` 精確刪除指定 bookmark
- 異常重複資料時 `delete` 報 conflict

## 不做的事

第一期不包含：

- 建立、重新命名、刪除 bookmark folder
- `list --recursive`
- 跨資料夾搬移 bookmark
- 自動將重名書籤改成 `(1)`、`(2)` 等後綴
- 嘗試修復既有損壞的 plist 資料

## 實作影響範圍

- `crates/cli/src/main.rs`
  - 新增 `SafariBookmarksAction`
  - 新增 `SafariAction::Bookmarks`
  - 補 dispatch 與 CLI parse tests
- `crates/adapter-macos/src/bookmarks.rs`
  - 新增書籤 CRUD 實作與測試
- `crates/adapter-macos/src/lib.rs`
  - 暴露 `pub mod bookmarks;`
- `README.md`
  - 補 Safari bookmarks 用法範例
- `Cargo.toml` / `crates/adapter-macos/Cargo.toml`
  - 新增 plist 解析依賴
