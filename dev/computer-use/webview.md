# Ordinary WKWebView background input probe

Prepared for issue [#32](https://github.com/Termdock-dev/cueward/issues/32), 2026-10-01.

```sh
python3 dev/computer-use/webview-probe.py --cli /absolute/path/to/cueward --inactive-space --output webview.json
```

Omit `--inactive-space` for a background receiver on the visible desktop. The inactive-Space case borrows an existing inactive user desktop and moves only its newly created window. It does not create, delete or switch desktops. The runner kills its own receiver and removes its snapshots and temporary files on exit. It checks the session before starting and refuses a locked desktop.

The fixture is an ordinary WKWebView with no hit-testing, event dispatch or first-mouse override. Its page contains one click counter and a draggable canvas object. Instrumentation observes DOM events and object/counter state; it does not inject events or modify native acceptance behavior. Coordinates come from current page rectangles and a new real snapshot for each trial. The CLI remains the sole input route.

The trials are one first click, one subsequent click on the same receiver, and one drag on a separate newly created receiver. Both click requests have count 1; there is no double-click fallback and no uncertain request is automatically resent. Each completed, failed or skipped trial is saved atomically before the next trial starts. If the first click activates its app, the second click is recorded as `skipped_foreground`; the runner does not reactivate or demote that app. A dispatch failure/interruption or observation error skips the remaining click as `skipped_previous_trial_error`. The first receiver is cleaned up before the independent drag begins, so its result cannot suppress drag evidence. Setup failures are also saved, and a report with errors/skips exits with code 1. `recorded` describes evidence collection, not acceptance success.

Reports distinguish posted events, actual click effects, DOM event button states, object displacement and release effects. A valid drag should move the object by 80 by 40 CSS pixels and finish with dragging released. A valid first click should add exactly one effect. The CLI waits for its synchronous drag helper to finish posting the release before returning; the additional 300 ms wait is receiver settling time after that return, not a wait overlapping the requested 500 ms drag.

The fixture samples activation, foreground PID changes and pointer difference at 10 ms intervals. It stores only counts, not unrelated app data. Timer sampling can miss transient changes, and physical pointer movements cannot be attributed to the agent. Continuous visible-Space sampling and foreground/interruption cases are still required for full #32/#31 acceptance. The move command's endpoint observations do not replace that coverage.

Swift compilation succeeded and the runner's cleanup paths executed. Desktop trials on 2026-10-01 stopped at capture, before any click or drag. A session probe then confirmed `CGSSessionScreenIsLocked: true` while Screen Recording access remained granted. These runs do **not** establish a WebView capture or input compatibility failure. Ordinary WebView first-click, subsequent-click, canvas displacement and release outcomes remain **unverified pending an unlocked run**. No product fix or support claim is included in this branch.

Run deterministic orchestration regressions without desktop access:

```sh
python3 -B -m unittest discover -s dev/computer-use -p test_webview_probe.py -v
```

These tests check checkpoints before later actions, first-click activation, uncertain/interrupted dispatch without replay, later snapshot failure, independent drag completion/release evidence, receiver cleanup and compilation failure reporting. Mocked processes and JSON state do not validate real WebView input compatibility.
