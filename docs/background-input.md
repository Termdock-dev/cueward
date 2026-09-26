# Background keyboard and pointer input

Use a recent window snapshot to address input to a background app, including a window on another Space. Cueward does not activate the app, switch desktops, post global input, move the system pointer, or use the clipboard to type text.

```sh
cueward window snapshot --id 12345
cueward window input-status --target '<input_target>'
cueward window type-text --target '<input_target>' --text 'Draft text'
cueward window key --target '<input_target>' --key tab
cueward window key --target '<input_target>' --key enter
cueward window key --target '<input_target>' --key a --modifiers command
cueward window scroll --target '<input_target>' --x 320 --y 240 --delta-y -240
cueward window click --target '<input_target>' --x 320 --y 240
cueward window click --target '<input_target>' --x 320 --y 240 --button right
cueward window click --target '<input_target>' --x 320 --y 240 --count 2
cueward window drag --target '<input_target>' --x 320 --y 240 --to-x 480 --to-y 320 --duration-ms 500
```

These commands require Accessibility and Screen Recording permissions and the Swift toolchain. Use the snapshot's `input_target`, not an AX node's `target`. Tokens expire after five minutes and bind the window ID, PID, title, frame, and image dimensions. They are observation references, not authorization credentials. A fresh token does not prove the contents or focus are unchanged.

Window frames use the catalog's integer-point precision, truncating fractional coordinates toward zero. Identity checks use that same precision; a sub-point movement that preserves those integer values is not detected.

## Keyboard routing

Process-directed keyboard events go to the app's current keyboard window. Cueward reads `AXFocusedWindow`, verifies its owning process and bounds, and uses an optional native AX window-ID lookup to match the requested capture window. This permits dialogs whose AX title is absent or differs from their system-catalog title. A failed lookup or conflicting ID is rejected. When the native lookup entry point is unavailable, Cueward requires the previous unique title-and-bounds match in the system catalog. Snapshot identity checks still require the catalog PID, title, and bounds to remain unchanged.

Cueward does not change the app's keyboard window to satisfy the request. The focused control inside that window determines where text goes; observe it before sending input. A dialog binding successfully does not establish that its controls will accept background input or that a save/open operation has completed.

`type-text` sends Unicode without replacing the clipboard. It accepts 1–1024 UTF-16 units, excluding control characters. Use `key` for Enter and Tab. Text goes through the app's event handling and may be rejected, transformed, or interpreted by the app.

Supported keys are `tab`, `enter`, `escape`, `backspace`, `delete` (forward delete), `left`, `right`, `up`, `down`, `home`, `end`, `page-up`, `page-down`, `space`, `a`–`z`, and `0`–`9`. Letter and number names designate US keyboard positions; the app and active keyboard layout determine shortcut interpretation. Modifiers are comma-separated `command`, `shift`, `control`, and `option`. Shortcut handling depends on the app's current responders and menus. Clipboard shortcuts still use the shared system clipboard.

## Scroll coordinates

`--x` and `--y` are pixels in the snapshot image, measured from its top-left corner. Cueward converts them using the captured frame and image size. Deltas use pixel units; positive values scroll up or left and negative values scroll down or right. Each delta must be within -4096 through 4096, and at least one must be nonzero.

Window-local wheel routing uses a dynamically discovered private macOS entry point and undocumented event fields. This route can change between OS releases. If the entry point is unavailable, Cueward returns an error without falling back to global input. The target app may ignore a correctly routed event.

## Clicks, drags, and dispatch readiness

`click` uses the same snapshot pixel coordinates and window-local routing as scroll. It supports `--button left` (default) or `right`, with `--count 1` (default) or `2`. A double click sends two pairs with click counts 1 then 2. It is a separate action with different effects; never use it automatically to retry a single click that had no visible effect. AppKit may replay a previously ignored first click while handling a double click.

`drag` holds the left button along a straight path within the captured window. `--duration-ms` defaults to 500 and accepts 50–2000; identity checks can make elapsed time longer. Both endpoints must lie inside the snapshot. All events are prepared before mouse-down. A detected interruption releases the button to the original receiver at the last delivered point and reports `partially_sent`. Abrupt process termination can prevent that release. This command does not establish support for system drag-and-drop sessions or cross-app file dragging.

`input-status` posts no events. It returns `keyboard` and `pointer` objects with `dispatch_ready` and an optional `reason`. The check includes current window identity, foreground conflict, keyboard-window binding, and availability of the pointer-routing entry point. `application_acceptance` remains `unverified`: a ready route does not prove that an app, canvas, or control will process the event. Recheck the actual effect after delivery.

`input_busy` reports whether another Cueward helper held the per-app input lock at the probe. A busy lock returns both routes as not ready instead of failing the status query. An idle probe releases its lock immediately; it does not reserve the app while AX checks run. This is a point-in-time observation, so readiness can change before the next command. Invalid or changed targets and permission errors still fail the query.

Controls that reject background first clicks can ignore a single click despite a ready route. Click, double-click, and drag handling can differ even within one app. If an action has no effect, take a new observation and select an action supported by the task and current interface.

## Interruption and verification

Input uses a private event source with explicit modifier flags. Concurrent raw-input commands to the same PID are rejected with a per-process file lock held by the event-posting helper. The lock stays held if the CLI caller exits while the helper is still alive. Before each event pair or wheel event, the helper checks whether its caller still exists and stops further input after detecting its exit. This does not lock out user input, other automation tools, or AX actions.

Before each key/click pair, wheel event, or drag step, Cueward rechecks token age, the window identity, and whether the target app is in the foreground. Keyboard actions also recheck the keyboard window. A target app in the foreground is rejected to reduce interference with user input. Switching to that app during a sequence stops further pairs or drag steps once detected, while a started drag still sends its release. These checks and delivery are separate operations; they cannot eliminate races with focus changes.

Each key-down is immediately followed by its prepared key-up without another AX call between them. Text delivery stops after a detected change or a 20-second execution budget. Abrupt process or OS termination can still interrupt event delivery.

Results contain `action`, `window_id`, `events_sent`, and a status:

- `sent_unverified`: the helper posted the events. The intended effect has not been checked.
- `partially_sent`: some events were posted before a detected interruption; `interruption` explains why.

`frontmost_pid_before`, `frontmost_pid_after`, and `foreground_changed` report endpoint observations. They do not detect every transient focus change or prove input isolation. Apps may activate themselves as a consequence of an action.

After input, take a new observation and check the actual task condition. On a partial result, error, or timeout, inspect before deciding whether to send more input. Automatically replaying text can duplicate input that was already delivered.

The opt-in desktop tests use disposable AppKit receivers to verify text, keys, shortcuts, non-key-window scrolling/clicks/drags, wrong-window rejection, dispatch status, and interrupted text/drag delivery with releases. They do not establish compatibility with every app. Caller-exit tests also verify that the helper retains its lock until it stops and releases a drag at the last delivered point. Direct helper termination can still interrupt release.

## Concurrent input evidence

A separate, consented experiment used a disposable AppKit foreground text receiver while another receiver accepted background text and shortcuts. All 74 physical key-downs reached the foreground receiver, including 27 during background posting; no background text appeared there. The background text matched the requested result, and 34 background Unicode key-downs posted while physical Shift was held retained zero modifier flags.

This establishes the observed isolation for those receivers and that run. It does not establish compatibility with every app, eliminate focus races, or provide a continuous focus/Space trace. Raw foreground typing and machine-specific diagnostics are not distributed.
