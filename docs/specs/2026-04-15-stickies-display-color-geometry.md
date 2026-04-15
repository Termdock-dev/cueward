# Stickies 進階顯示控制

## Goal

處理 issue #98，為 `cueward stickies` 補上便條紙的進階顯示控制：

- 指定螢幕
- 顏色
- 尺寸
- 座標 / 位置

維持既有資料檔路線，不改走 GUI scripting。

## POC 結論

2026-04-15 本機驗證已確認：

- `.SavedStickiesState` 會驅動 Stickies 視覺狀態
- 修改 `Frame` + `ExpandedSize`，重開 Stickies 後，便條會真的移動 / 改變大小
- 將 `Frame` 改到目標螢幕的全域座標區域後，便條會出現在該螢幕
- 顏色不是單一欄位，而是至少同時受以下四組欄位控制：
  - `ControlColor`
  - `HighlightColor`
  - `SpineColor`
  - `StickyColor`
- 六個內建色盤都可由 state 回推
- 直接寫入非 preset 的自訂 RGBA 也會生效

代表 issue #98 的三個核心能力都可沿用資料檔路線完成。

## Scope

### In scope

- `stickies create` 支援：
  - `--display`
  - `--color`
  - `--x --y`
  - `--width --height`
- `stickies update` 支援：
  - `--color`
  - `--x --y`
  - `--width --height`
  - `--display`
- 以 typed parser / formatter 控制 `Frame` 與 `ExpandedSize`
- 以 preset model 控制六種常見顏色：
  - `yellow`
  - `blue`
  - `green`
  - `pink`
  - `purple`
  - `gray`

### Out of scope

- 富文字格式
- 字型 / 字距 / 字級
- 置頂 / 半透明 / 預設值等其他視窗選項
- 第一版 CLI 直接暴露任意 `#RRGGBB`

註：任意自訂色已證實技術上可行，但第一版先以 preset 收斂 scope。若實作過程保持簡單，可視情況拆下一張 issue 再加。

## CLI 設計

### Create

```bash
cueward stickies create \
  --title "臨時待辦" \
  --body "內容" \
  --display 2 \
  --color blue \
  --x 120 \
  --y 80 \
  --width 420 \
  --height 260
```

### Update

```bash
cueward stickies update \
  --id <sticky-id> \
  --display 3 \
  --color gray \
  --x 40 \
  --y 60 \
  --width 360 \
  --height 220
```

## 語意定義

### `--display`

- 1-based 編號
- 語意與 repo 內既有 `screenshot --display <n>` 對齊
- 本質不是 state 裡有 `display_id`
- adapter 會先取得該螢幕的 frame / visible frame，再把便條位置映射成全域桌面座標

### `--x --y`

- 兩者必須成對出現
- 若同時指定 `--display`：
  - `x` / `y` 視為「目標螢幕內的相對座標」
  - adapter 會轉成全域座標後寫回 `Frame`
- 若未指定 `--display`：
  - `x` / `y` 視為全域桌面座標

這樣 `display` 與 `x/y` 是可組合語意，不是互相覆蓋。

### `--width --height`

- 兩者必須成對出現
- 寫回：
  - `Frame`
  - `ExpandedSize`

避免 view state 與 metadata 分裂。

### `--color`

- 第一版支援六色 enum
- 每個 enum 會映射成四組 RGBA dictionary，一次寫回：
  - `ControlColor`
  - `HighlightColor`
  - `SpineColor`
  - `StickyColor`

## 色盤模型

POC 已挖到完整 preset。

### blue

- `ControlColor`: `(0.141176, 0.815686, 0.913725, 1.0)`
- `HighlightColor`: `(0.007843, 0.737255, 0.843137, 1.0)`
- `SpineColor`: `(0.537255, 0.941176, 1.0, 1.0)`
- `StickyColor`: `(0.678431, 0.956863, 1.0, 1.0)`

### yellow

- `ControlColor`: `(0.858824, 0.772549, 0.011765, 1.0)`
- `HighlightColor`: `(0.737255, 0.662745, 0.007843, 1.0)`
- `SpineColor`: `(0.996078, 0.917647, 0.239216, 1.0)`
- `StickyColor`: `(0.996078, 0.956863, 0.611765, 1.0)`

### green

- `ControlColor`: `(0.317647, 0.733333, 0.317647, 1.0)`
- `HighlightColor`: `(0.282353, 0.635294, 0.282353, 1.0)`
- `SpineColor`: `(0.513725, 0.996078, 0.513725, 1.0)`
- `StickyColor`: `(0.698039, 1.0, 0.631373, 1.0)`

### pink

- `ControlColor`: `(0.972549, 0.498039, 0.498039, 1.0)`
- `HighlightColor`: `(0.886275, 0.458824, 0.458824, 1.0)`
- `SpineColor`: `(1.0, 0.698039, 0.698039, 1.0)`
- `StickyColor`: `(1.0, 0.780392, 0.780392, 1.0)`

### purple

- `ControlColor`: `(0.490196, 0.607843, 0.921569, 1.0)`
- `HighlightColor`: `(0.458824, 0.568627, 0.862745, 1.0)`
- `SpineColor`: `(0.607843, 0.713725, 0.996078, 1.0)`
- `StickyColor`: `(0.713725, 0.792157, 1.0, 1.0)`

### gray

- `ControlColor`: `(0.658824, 0.658824, 0.658824, 1.0)`
- `HighlightColor`: `(0.619608, 0.619608, 0.619608, 1.0)`
- `SpineColor`: `(0.854902, 0.854902, 0.854902, 1.0)`
- `StickyColor`: `(0.933333, 0.933333, 0.933333, 1.0)`

註：tuple 順序為 `(Red, Green, Blue, Alpha)`。

## 模組切分

目前 `crates/adapter-macos/src/stickies.rs` 已接近 repo 單檔上限。做 #98 前應先拆成目錄模組。

建議結構：

```text
crates/adapter-macos/src/stickies/
  mod.rs
  state.rs       # plist 讀寫、entry model
  geometry.rs    # Frame / ExpandedSize parser 與 formatter
  color.rs       # 色盤 model 與 dict mapping
  display.rs     # 螢幕資訊讀取與 display -> global frame 映射
  tests.rs
```

CLI 仍維持：

- `crates/cli/src/commands/stickies.rs`
- `crates/cli/src/commands/stickies_tests.rs`

## 實作策略

### Geometry

- 建立 typed `StickyFrame` / `StickySize`
- 封裝：
  - `parse_frame("{{x, y}, {w, h}}")`
  - `format_frame()`
  - `parse_size("{w, h}")`
  - `format_size()`
- `update` 與 `create` 共用同一組 geometry helper

### Display

- 取得目前螢幕列表與 frame / visible frame
- 建立 `DisplayTarget` model
- `--display <n>` 轉為目標螢幕的 anchor frame
- 預設位置不再只做固定 `offset_frame()`，而是以目標螢幕為單位做 cascade，避免多張便條永遠疊在同一個點

### Color

- `StickyColorPreset` enum
- 每個 preset 對應一組 `StickyColorScheme`
- 寫回時一次更新四個 color 欄位

## 錯誤處理

至少明確區分：

- `invalid display number`
- `display not found`
- `x and y must be provided together`
- `width and height must be provided together`
- `invalid frame string`
- `invalid expanded size string`
- `unsupported sticky color`
- `sticky not found`

## 測試

### CLI

- `stickies create --display --color --x --y --width --height`
- `stickies update --id --display --color --x --y --width --height`
- 拒絕半套 geometry 參數

### Adapter

- `Frame` parse / format round-trip
- `ExpandedSize` parse / format round-trip
- preset -> four dictionaries mapping
- `display + relative x/y` 轉成 global frame
- `create` / `update` 正確寫回 `Frame` / `ExpandedSize`
- `create` / `update` 正確寫回四個 color dict

### Manual smoke

- 主螢幕 create
- 外接螢幕 create with `--display`
- update color to one preset
- 多張 create 不會疊在完全同一點

## 驗收條件

- `create` 可穩定指定螢幕
- `create` / `update` 可穩定指定六種顏色之一
- `create` / `update` 可控制位置與尺寸
- 同一螢幕多張便條不會永遠疊在同一位置
- 實作維持資料檔路線，不依賴 GUI scripting
