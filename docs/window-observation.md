# Window discovery and image snapshots

Use window discovery and snapshots to observe application windows without activating their apps or switching desktops. These commands do not require the app to expose Accessibility elements.

```sh
# List titled application windows on visible desktops.
cueward window list

# Include windows outside visible desktops.
cueward window list --all-spaces

# Capture one exact window, including a window on another Space.
cueward window snapshot --id 12345 --output frame.png

# Include OCR text from the same image.
cueward window snapshot --id 12345 --ocr
```

## Requirements and window selection

The commands require macOS, `swift` on PATH, and permission to read window metadata and capture the screen. Enable Screen Recording access for the terminal app running Cueward when requested by macOS.

The catalog includes titled, nontransparent application windows with positive dimensions at the normal window layer. System overlays and untitled windows are excluded. Results are sorted by the frontmost owning app, then app name, title, and window ID.

`--all-spaces` includes non-visible candidates reported by the system. `is_onscreen: false` may mean another Space, a hidden app, or a minimized window. It does not identify a particular Space or guarantee the window can be captured. `is_frontmost` describes the owning app, not whether this exact window is foremost.

Use the returned `window_id` for `window snapshot`. Snapshot lookup includes non-visible windows automatically. Neither command moves windows, creates desktops, activates an app, or injects input.

## Snapshot result and coordinates

Output is JSON inside Cueward's `<external>` data wrapper. A snapshot contains:

- `window`: ID, owner PID, app, title, visibility, and window-frame bounds.
- `screenshot`: PNG path, capture timestamp, and optional OCR text.
- `image`: pixel `width` and `height`, `scale_x` and `scale_y`, and `origin: "window_frame_top_left"`.
- `input_target`: a five-minute reference for [background keyboard input and scrolling](background-input.md), bound to the observed window and image dimensions.

The PNG excludes the window shadow and attached windows such as sheets. It includes the selected window frame and its title bar. Image coordinates start at the image's top-left corner. Window bounds use macOS global screen coordinates in points, with a top-left origin; coordinates can be negative on other displays.

Convert an image point `(image_x, image_y)` to global screen points as follows:

```text
screen_x = window.bounds.x + image_x / image.scale_x
screen_y = window.bounds.y + image_y / image.scale_y
```

For example, a window frame of 400 by 300 points with an 800 by 600 pixel PNG has scale factors of 2. An image point of `(200,100)` lies `(100,50)` points from the window frame's top-left corner. The content area below the title bar has a different origin; do not treat image coordinates as content coordinates.

The mapping describes the captured frame. A later window move, resize, or display configuration change requires a new snapshot before using its coordinates.

## Consistency and files

Cueward looks up the exact ID, captures to a temporary PNG, reads its dimensions, and rechecks the window's ID, owner PID, title, and bounds. A disappeared or changed target returns an error. The destination file is replaced only after these checks succeed; a failed capture or identity check preserves an existing destination file.

Without `--output`, each successful snapshot uses a unique PNG filename in Cueward's screenshot cache. With `--output`, its parent directory must already exist. PNG encoding is used regardless of the filename extension.

These are before-and-after checks, not an atomic WindowServer snapshot. They cannot detect a replacement with identical observable attributes or a change that reverses between checks. Visibility and foreground changes do not invalidate an otherwise unchanged window frame. The returned visibility describes the final catalog read.

OCR follows the existing screenshot behavior: no sufficiently confident text means `ocr_text` is omitted. A reported OCR error emits a warning while retaining the image.

## Accessibility commands

`window snapshot` produces an image observation and a window-level `input_target`. It does not produce AX element targets. `window inspect`, `window press`, and `window set-value` retain their existing on-screen Accessibility contract. A window appearing in `window list --all-spaces` does not imply that those commands can operate it.

`screenshot windows` continues to list on-screen windows. Its results now also include `is_onscreen`. Existing `screenshot` image-capture commands retain their behavior.
