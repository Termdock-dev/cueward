# 教訓與改進 (Lessons Learned)

## 架構視野與平台依賴
- **錯誤**: 在規劃初期急於實作 macOS 的功能，忽略了未來跨平台 (Windows) 的可能性。
- **改進**: 任何系統級別的基礎工具都必須在第一天採用「核心邏輯與平台適配器分離 (Adapter Pattern)」的設計。不讓底層 API (如 AppleScript 或 SQLite) 汙染核心領域模型 (Domain Model)。

## 現代工具鏈與品味
- **錯誤**: 在 2026 年依然下意識地將 Rust Edition 降級回 2021。在專業專案文件 (README) 中使用過多 Emoji。
- **改進**: 必須時刻保持對現代工具鏈的認知。採用最新穩定標準 (Edition 2024) 才是負責任的工程品味。文件與程式碼應依靠清晰的排版、留白與精練的文字來展現力量，拒絕廉價的 AI 生成感。

## 網路請求與爬蟲 (前期紀錄)
- **錯誤**: 輕易假設可以透過 HTTP 直接抓取受保護的社群連結 (如 Threads)。
- **改進**: 如實告知系統無法讀取，優先使用本地可信任資料或要求人類介入，避免產生幻覺 (Hallucination)。

## SQLite 權限限制 (前期紀錄)
- **錯誤**: 輕視 macOS TCC (Transparency, Consent, and Control) 保護機制，導致讀取 SQLite 失敗。
- **改進**: 必須在腳本或 CLI 執行前，預先處理並明確提醒使用者授予「全磁碟存取 (Full Disk Access)」權限。
