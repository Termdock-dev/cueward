# Screenshot Window Capture

## Goal

處理 issue #101，為 `cueward screenshot` 補上指定視窗截圖能力。

需求重點：

- 保留現有 `cueward screenshot` 全螢幕截圖
- 新增 `cueward screenshot windows` 列出可截圖視窗
- 新增 `cueward screenshot window --id <id>` 截指定視窗
- 視窗截圖也支援既有 `--ocr`、`--output`

## Scope

### In scope

- `cueward screenshot`
- `cueward screenshot --ocr`
- `cueward screenshot --output <path>`
- `cueward screenshot windows`
- `cueward screenshot window --id <id>`
- `cueward screenshot window --id <id> --ocr`
- `cueward screenshot window --id <id> --output <path>`

### Out of scope

- `--window <text>` title 模糊匹配
- 互動式選單
- 人類可讀輸出
- 多視窗自動選最前景

第一版只做 `list -> choose id -> capture`。

## CLI 設計

### 保留現有全螢幕行為

```bash
cueward screenshot
cueward screenshot --ocr
cueward screenshot --output out.png
cueward screenshot --display 2
```

### 列出可截圖視窗

```bash
cueward screenshot windows
```

輸出 JSON 陣列：

```json
[
  {
    "window_id": 12345,
    "app": "Discord",
    "title": "工程室",
    "owner_pid": 987,
    "is_frontmost": true,
    "bounds": {
      "x": 120,
      "y": 80,
      "width": 1440,
      "height": 900
    }
  }
]
```

### 截指定視窗

```bash
cueward screenshot window --id 12345
cueward screenshot window --id 12345 --ocr
cueward screenshot window --id 12345 --output window.png
```

## 架構

現有 `screenshot` 功能集中在：

- `crates/adapter-macos/src/screenshot.rs`
- `crates/cli/src/commands/screenshot.rs`
- `crates/cli/src/commands/mod.rs`

這張 issue 不需要新 crate，也不需要碰 `core`。

建議拆成：

```text
crates/adapter-macos/src/screenshot/
  mod.rs
  capture.rs      # screencapture command builder / file output / OCR flow
  windows.rs      # CGWindowListCopyWindowInfo bridge + filtering + sorting
  tests.rs
```

原因：

- 現有 `screenshot.rs` 已含 output path、OCR、display capture
- 加 `window list` + `window capture` 後責任會變多
- repo 規則傾向小模組、避免單檔膨脹

## 視窗來源

視窗列表走 macOS 原生 `CGWindowListCopyWindowInfo`。

需要萃取：

- `kCGWindowNumber`
- `kCGWindowOwnerName`
- `kCGWindowName`
- `kCGWindowOwnerPID`
- `kCGWindowBounds`
- `kCGWindowLayer`
- `kCGWindowAlpha`
- `kCGWindowIsOnscreen`

可用方式：

- Swift / AppKit / CoreGraphics 小片段，透過 `swift -e`
- 或 Objective-C bridge

第一版優先選最少依賴、最容易測的路線。

## 可截圖視窗過濾規則

`screenshot windows` 只列「可截圖視窗」。

至少過濾：

- 有 `window_id`
- `bounds.width > 0`
- `bounds.height > 0`
- `is_onscreen == true`
- `alpha > 0`
- 排除非一般內容窗的高 layer 項目

預設也應排除：

- 空 app 名稱
- 明顯沒有 title / 尺寸的系統雜訊窗

## 排序規則

- 最前景 app 的視窗優先
- 其餘再按：
  - `app`
  - `title`
  - `window_id`

`is_frontmost` 需要一起輸出，讓 agent 好選。

## 視窗截圖實作

指定視窗截圖走：

```bash
screencapture -x -l <window_id> <path>
```

延續現有流程：

- 沿用 `--output`
- 沿用 `--ocr`
- 成功後仍回 `ScreenshotResult`

與現有 `--display` 的關係：

- 全螢幕模式：`--display`
- 視窗模式：`window --id`
- 第一版不支援 `window --id` 和 `--display` 混用

## 錯誤處理

至少明確區分：

- `invalid window id`
- `window id not found`
- `window capture failed`
- `no capturable windows found`
- `failed to list windows`
- `path must not contain parent directory components`

## 測試

### CLI

- `cueward screenshot`
- `cueward screenshot windows`
- `cueward screenshot window --id 123`
- `cueward screenshot window --id 123 --ocr`
- `cueward screenshot window --id 123 --output out.png`

### Adapter

- 視窗列表 parser 能讀出：
  - `window_id`
  - `app`
  - `title`
  - `owner_pid`
  - `bounds`
- 過濾規則排除不可截圖視窗
- 排序把前景視窗放前面
- `screencapture -l` 指令組裝正確
- `window capture` 與既有 `display capture` 不互相干擾

### Manual smoke

- `screenshot windows` 能列出目前可見 app 視窗
- `screenshot window --id <id>` 可成功截圖
- `--ocr` 在 window mode 可正常回傳文字
- `--output` 在 window mode 可正常落檔

## 驗收條件

- agent 可先列出可截圖視窗，再用 `window_id` 截指定視窗
- `cueward screenshot` 舊用法不破壞
- `window --id` 可和 `--ocr` / `--output` 一起用
- 預設輸出為 JSON，不增加人類格式分支
