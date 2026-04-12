# Safari Automation Guard

## Goal

處理 issue #57，為 `crates/adapter-macos/src/safari.rs` 增加共用保護層，降低 Safari 自動化被平台偵測與多 process 競爭的風險。

## Decision

採用 adapter 內的共用 guard，而不是在 CLI 分支或每個函式分散實作。

原因：

- 符合 DRY，所有 Safari automation 共用同一套節流與 lock。
- 未來若其他入口直接重用 adapter，也會自動繼承保護層。
- 變更集中在 `safari.rs`，不需要把 retry/lock 邏輯散落到 CLI。

## Scope

1. 固定節流：每次 Safari automation 底層操作之間至少等待 1 秒。
2. 429 退避：若底層結果或錯誤文字可辨識為 rate limit，最多重試 3 次，等待 30/60/90 秒。
3. File lock：使用 `~/.cueward/lock.json`，TTL 1800 秒，避免多個 process 同時操控 Safari。

## Notes

- lock 採最小可行設計：以 TTL 作為 crash 後的自動恢復機制，不額外依賴 process liveness 檢查；TTL 需覆蓋最長 Safari 任務。
- 429 偵測僅針對明確且短訊號，例如 `Too Many Requests`、`HTTP 429`、`"status":429` 等，避免過度誤判一般內容。
