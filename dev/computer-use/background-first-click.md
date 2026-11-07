# Background first click

Recorded 2026-11-06 (Asia/Taipei) on macOS 27.0.1, arm64, three displays. The [aggregate](results/2026-11-06-background-first-click.json) holds the counts. Production `window click` and `window drag` now prepare the target as described in [background input](../../docs/background-input.md#click-preparation).

## Why first clicks failed

The receiver's event log shows the order on an inactive app:

| Offset | Event | Window key | App active |
| --- | --- | --- | --- |
| 0 ms | ApplicationActivated from the focus record | no | no |
| 51 ms | Mouse down arrives | no | yes |
| 52 ms | AppKit makes the window key while handling that down | yes | yes |

`acceptsFirstMouse` returns false, so the view never sees the click. A window that only becomes key during a click still counts that click as first mouse. The window has to be key before the down, which is why one click is spent on the title bar.

`AXFocusedWindow` cannot replace that click. It named the target while native `isKeyWindow` was still false, and skipping the title bar click on that evidence left the view with no events.

## Contrast with Codex Computer Use

Codex Computer Use 26.1027 ran on the same receiver through `codex exec`'s interactive route, after the owner approved the test app. Its receiver log shows the same sequence: ApplicationActivated, a title bar click at (30, 16), and the real click 9 ms later, with the same window routing fields and a source PID of its service. Element-index clicks also arrived as pointer events. It kept the target active for 23 seconds, and its canvas drag failed the same way.

Its title bar point sits between the close and minimize buttons on most windows, but positions differ. LINE places them at x = 5, 23 and 41, so (30, 16) lands on minimize there. Cueward measures the gap through Accessibility for that reason.

## Results

| Case | Result |
| --- | --- |
| Pre-fix live test, view refusing first mouse | Zero view events |
| Production CLI, ordinary WKWebView, visible Space | First and later single clicks passed |
| Production CLI, ordinary WKWebView, inactive Space | First and later single clicks passed; desktop check passed |
| Production CLI, canvas drag, both contexts | Failed: 27 events sent, `event.buttons` 0, object unchanged |
| 40-second soak | 14 of 14 clicks exact; target key for 209 ms on average; 11 physical keys landed during those windows and 0 of 152 reached the target |
| Inactive Space, regular activation policy | 3 of 3 clicks each; no foreground, menu bar or Space change at 20 ms sampling |
| Make-key record pair | Rejected: the receiver stopped handling events and stayed active |

Held-button state follows the physical button. One drag appeared to pass with `buttons` 1, and the trace showed a person holding the mouse through it: 79 physical drag events in the interval. With no physical press, `pressed` stayed 0 in five of five runs.

## Real apps

Stage Manager was off, and each app was started by the test. Foreground, menu bar and Space stayed unchanged at 20 ms sampling during the clicks, with no synthetic event on the global stream.

| App | Action | Result |
| --- | --- | --- |
| Calculator | Clicks on 7, 8 and 9 | Display showed exactly `789` |
| TextEdit | Click inside text, then type `X` | Inserted at the clicked position |
| TextEdit | Drag selection, then type `Y` | Selected text replaced |
| Finder | Click on a file name | Row selected; path bar showed the file |
| Safari | Three clicks on a page button | Title `clicked-1`, `clicked-2`, `clicked-3` |
| Chrome, temporary profile | Three clicks on a page button | Title `clicked-1`, `clicked-2`, `clicked-3` |

Chrome first failed with `no_safe_point`. Its Accessibility title was `cueward-web-probe - Google Chrome` against a catalog title of `cueward-web-probe`, so title matching found no window and `window inspect` failed too. Window binding now compares the native window ID first.

Chrome ignores `open -g` and comes to the foreground, where the foreground guard correctly refuses input. Calculator opened with `open -g` under Stage Manager showed no usable window.

## Delivery on exit

A helper that exits right after `postToPid` loses its events: 0 of 40 arrived, against 40 of 40 with a 170 ms wait. This explains the precompiled helper's missing pair in the [benchmark notes](README.md). The interpreted helper hid it through its slow teardown. The helper now waits 120 ms after sending any event.

## Limits

- Real Electron apps were not tested, because every one on this machine holds user data. Chrome is the nearest coverage.
- With Stage Manager on, the catalog showed thumbnail-sized windows such as 94 by 104 points while Accessibility reported 673 by 439. Turning it off removed the difference. The [Stage Manager measurement](stage-manager.md) found that pointer input hit the wrong control there, so it is now refused.
- Applications that react to did-become-active notifications will see each click.
- Sampling at 20 ms cannot exclude shorter transients.
