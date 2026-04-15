# Notes Drawing Attachments

## Goal

處理 issue #16，讓 Apple Notes 的 drawing / sketch 類附件不再落入 unresolved 黑箱。

## Scope

### In scope

- 調查 drawing / sketch 在 Notes DB 的最小可辨識訊號
- 對 drawing 類附件建立明確 typed output
- 若暫時不能提取實體內容，至少標成 `kind = drawing`
- 必要時補最少 debug / placeholder 資訊，避免靜默吞掉

### Out of scope

- drawing 內容完整匯出
- 向量化 / stroke reconstruction
- 手寫辨識
- drawing 縮圖生成

## Current State

- `AttachmentKind::Drawing` 已存在於 core schema
- notes attachment pipeline 目前已支援 image / web preview / map / file-backed / audio
- 但沒有 drawing resolver
- 結果是 drawing 很可能仍落入 unresolved 或一般 media 路徑

## Desired Behavior

當 Notes 內存在 drawing / sketch 類附件時：

1. pipeline 能辨識它是 drawing，而不是 unresolved
2. `attachment_segments` 會輸出 `kind = drawing`
3. 若目前沒有穩定的實體檔或內容表示：
   - 仍保留 `kind = drawing`
   - 不假裝是 image / binary / unresolved

## Architecture

採最小增量做法：

- 在 notes DB layer 補 drawing 類附件 loader
- 在 notes attachment pipeline 新增 drawing resolver
- 不改既有 CLI 形狀
- 不新增新的 top-level command

新增的資料流：

1. DB layer 查出 drawing attachment rows
2. 以 note timestamp/title 與主 note 對齊
3. attachment pipeline 在 unresolved fallback 之前接入 drawing resolver
4. 輸出 typed `drawing` segment

## Detection Strategy

第一版以本機資料為準，不先猜完整 Apple 私有格式。

調查重點：

- `attachment.ZTYPEUTI`
- 相關 `ZMEDIA` / `ZFILENAME` / `ZIDENTIFIER`
- 是否存在可穩定辨識的 drawing UTI
- 若沒有明確 UTI，是否有可重現的其他欄位組合

第一版偏好：

- 有穩定訊號才 classify 成 drawing
- 沒有穩定訊號就維持現況，不亂判

## Output Contract

第一版 drawing segment 目標：

```json
{
  "index": 1,
  "kind": "drawing"
}
```

若後續能穩定取得檔名 / path / sha256，再加欄位；第一版不強求。

## Acceptance Criteria

- drawing 不再落入 unresolved 黑箱
- `attachment_segments` 可輸出 `kind = drawing`
- 既有 image / web preview / map / file-backed / audio 行為不回歸
- CLI / JSON shape 維持向後相容

## Risks

- Apple Notes drawing 可能沒有穩定公開檔案表示
- drawing 與一般 image/file attachment 的區分可能需要依賴私有欄位
- 若訊號不穩，寧可保守不分類，也不要誤判成 drawing
