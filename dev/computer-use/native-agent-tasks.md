# Fresh native document task execution

This continues [#36](https://github.com/Termdock-dev/cueward/issues/36) with runnable owned `new_document`, `existing_document`, `cross_app`, `calculation`, `window_dialog` and [WebView pointer](#webview-pointer-tasks) tasks. The [document/calculation aggregate](results/2026-10-04-native-agent.json) records successful post-fix disk results, but **zero full task passes** because foreground and pointer observations changed. The [2026-11-06 aggregate](results/2026-11-06-native-agent.json) records the first full task pass, `calculation`, after the observer gained [input attribution](desktop-observer.md). The [window/dialog aggregate](results/2026-10-05-window-dialog-agent.json) retains a failed initial attempt and a locked-session prerequisite check. #36 remains incomplete; #33 is not repaired by this change.

## Product defect found during execution

An ordinary `NSTextView` without an explicit accessibility identifier returned AX `.failure` for `AXIdentifier`. Cueward read its value correctly, but `app_common.swift` then cleared independently read actions and `settable_value`, leaving no usable app-scoped edit target. The pre-fix fresh Agent opened the document but could not edit or save it.

The product fix treats unavailable `AXIdentifier` as optional metadata, alongside the existing `AXDescription` exception. The unavailable marker remains part of the target fingerprint, so availability changes invalidate old targets, including changes after lock acquisition. Other attribute failures, secure-value classification, disabled controls, process identity, foreground and locked-session protections remain unchanged. Communication errors such as `.cannotComplete` still fail inspection. No identifier was added to the fixture to evade the defect.

Test-only commit `bfc65e0` reproduces both inspection and action failures: zero passed, two failed, exit 101. Fix commit `9e5338d` makes the accessibility suite pass: 19 passed, one ignored. Release build passed; the full serial Rust suite passed 916 tests with 33 ignored, and initial Python validation passed 148 tests. The fixture and observer compiled under strict Swift 6 with warnings as errors. Scoped Rust formatting passed; workspace-wide formatting still reports pre-existing differences outside these changes. Review follow-up validation is recorded below.

## Run and hand off

Use an unlocked macOS desktop with Accessibility and Screen Recording access. Prepare a new run, never reuse a failed task's receiver, workspace or evidence:

```sh
cargo build --release
run=$(python3 -B dev/computer-use/task-acceptance.py prepare --parent /tmp | \
  python3 -c 'import json,sys; print(json.load(sys.stdin)["run"])')
socketdir=$(mktemp -d /tmp/cwa.XXXXXX)
python3 -B dev/computer-use/native-agent-task.py \
  --cli "$PWD/target/release/cueward" --run "$run" \
  --task existing_document --socket "$socketdir/a.sock"
```

The runner stays in the operator's terminal. Wait for broker `ready`, then give a new Agent only the goal and owned scope from `operator/existing_document/owned-scope.json`, the [generic app documentation](../../docs/app-exploration.md), and the [socket client contract](agent-session.md). Do not supply fixture source, receiver state, manifest baselines, old targets or previous attempts. The Agent chooses its observations/actions and sends `--finish` through `agent-command.py`. The operator separately reads final state and disk results. Agent self-reported request totals are not authoritative; use the checkpointed broker count and CLI receipts.

For `calculation`, use another fresh Agent and socket directory with `--task calculation`. Both tasks may share one prepared run, but each gets a new receiver PID and unique bundle. Rerunning the same task requires another prepared run. The calculation setup creates a blank named file, not the answer. The general goal permits a calculator or document app, but this runner scopes execution only to its owned document fixture and validates that route; it cannot demonstrate Calculator UI operation.

The fixture starts with an unopened, blank document and zero reads. Its ordinary owned-file Open button performs `NSDocument.read`, so opening is an Agent action, not preloaded setup. Editing uses an ordinary text view and saving uses `NSDocument.save`. This button-bound owned destination is not native file-dialog or first-save-panel coverage. The fixture has no first-mouse, focus, activation or input-acceptance override. Operator binaries, state and logs stay outside the task workspace.

The unchanged scoped broker permits only this receiver/window, uses a 0600 socket and retains the corrected external-content boundary. It does not permit arbitrary app opening, Space mutations or global input. The broker is bounded to 800 seconds and 64 requests; receiver and observer deadlines are 900 seconds. Cleanup uses retained child handles, unregisters only the uniquely built bundle and removes the owned socket file. Cleanup failures, including nonzero bundle unregistration, are recorded and cause a nonzero runner exit. After the runner exits, remove its empty socket directory with `rmdir "$socketdir"`. Preserve raw evidence privately before any manual cleanup of the prepared run.

Before reserving metadata or launching, the runner rejects an existing copy whose bytes differ from its prepared baseline. A usable independent receiver record requires the exact owned file, positive integer read/save/write counts, no receiver error and a cleared document dirty state. Missing or contradictory evidence cannot become a passing receiver just because disk bytes happen to match. If a started transport or final receiver observation fails, verified checkpoint metadata/receipts/trace remain an attempted record without a fabricated receiver observer; its effect remains failed or unverified, not `not run`. Raw session/error files remain private. Invalid or unfinished checkpoint identity is retained as an error, never invented as a verified attempt.

After the desktop-observer context closes, the runner reads a new sequence baseline and waits for a newer receiver snapshot, bound to the retained PID and initial native window ID. It never substitutes the startup sequence or a reread of the same JSON. Missing, noninteger, regressing or frozen sequences and changed identities fail the final read. The fixture only adds a read-only snapshot sequence; its activation counter is never reset during the process. Evidence retains `active`, `activations`, `window_id`, `observation_sequence` and the operator's `fresh_after_sequence` baseline. The evaluator fails a current activation or any recorded earlier activation, including `active: false, activations: 1`. Without an established activation, malformed or unfresh supplemental state stays unverified. These fields supplement, not replace, the sampled desktop trace.

The runner's exit 0 means execution/checkpointing completed, not that all eight acceptance tasks passed. Inspect `native-agent-report.json` or run the independent checker:

```sh
python3 -B dev/computer-use/task-acceptance.py evaluate --run "$run" \
  --evidence "$run/native-agent-evidence.json" --output "$run/checked-report.json"
```

The checker exits 1 while any task is failed or unverified. Task records cannot overwrite an earlier attempt; all eight tasks stay in the denominator.

## Two-window native save-dialog task

Prepare a new run and socket directory, then use `--task window_dialog`. Its setup opens two existing disposable documents through normal `NSDocument.read`; the Agent performs editing and the native Save As interaction, not initial opening. The goal supplies the requested text and exact target path. Give a fresh Agent only that goal, owned PID/two native window IDs, generic app documentation and the broker contract. No fixture, observer state, expected-result manifest or previous attempt belongs in its handoff.

The fixture uses an ordinary `NSTextView` and `NSDocument.saveAs`. It has no identifier on the editor, first-mouse, focus, activation or input-acceptance override. The panel's filename and Save enabled state remain native. It disables New Folder, refuses document relocation and restricts read/write destinations to the two prepared regular files, with only target writes authorized. These fixture restrictions limit authorized side effects; they are not a filesystem sandbox or a fix for first-save acceptance. A native panel may still expose unrelated read-only navigation metadata. This synthetic existing-document task does not reproduce #33's inactive first-save setup.

The independent helper associates document UUID identifiers with `_AXUIElementGetWindow` native IDs. A remote save sheet does not expose the configured panel UUID: it can report `save-panel` and belong to the shared save-panel service, or contain service-owned descendants under a host-owned sheet. The observer instead requires the exact panel native ID and `AXWindow` equality with the owned target document, retaining the current descendant ref and actual receiver PIDs. It never selects a sheet by title or fixed `wN` index.

The broker permits only bounded app inspection/press/value assignment and the owned windows' snapshot/inspection. It rejects menus, global/pointer actions, unknown/duplicate/attached flags and output paths. Action targets must have been returned by a current broker observation. Document roots stay restricted to the host PID; independently bound service targets are permitted only under that exact owned panel. Every observed panel receiver is checked, including groups without action targets. A foreign receiver with missing/wrong native panel identity stops binding instead of disappearing from coverage. Context changes invalidate the target registry. A CLI return followed by an evidence failure is an uncertain dispatched outcome, never `not_dispatched` or permission to replay.

Actual Agent inspections/actions supply the `initial`, `dialog`, `resumed` phases in the desktop observer's timebase. Setup observations cannot substitute for them. Final evidence independently checks normal target save/write, no rejected writes, cleared dirty state, untouched bystander IO/content, exact disk bytes and a fresh inactive receiver snapshot.

Unlike `new_document`, the `window_dialog` goal has no exact-once save-submission condition. Its native IO check requires positive integer read/save/write counts. The fixture increments `save_requests` when `save` or `saveAs` is invoked, before the panel or write completes, so the count is not a count of successful disk saves. This does not authorize blindly replaying an uncertain action; the Agent must observe before any new decision, and actual receipts remain available for review.

The shared save-panel service is an auxiliary receiver, not a newly owned process. Cleanup never terminates it. The [evaluator contract](task-acceptance.md#shared-save-panel-receivers) requires auxiliary activation coverage across the entire execution interval. The current desktop observer starts with the host alone; late discovery of a service cannot supply missing earlier samples. Such service coverage stays `unverified`, even if panel traversal and disk save eventually succeed. Standalone service-owned panels are not implemented by this runner.

### Recorded window/dialog work: 2026-10-05

The initial fresh Agent dispatched five CLI commands. It edited and read back the requested Unicode text, then opened Save As. The runner's original panel-UUID assumption failed during post-action observation; transport closed and the Agent stopped without replay. Disk target bytes remained unchanged, the bystander/source hashes were preserved, and only the initial phase was recorded. The report remains **0 passed, 1 failed, 7 unverified**, with artifact/interference failures and missing complete receiver evidence. Foreground and pointer changes remain unattributed and unfiltered. Owned receiver activation remained zero. Cleanup stopped the retained host and unregistered its unique bundle.

Separate operator diagnostics established the native sheet association and remote receiver behavior, leading to the fail-closed binding implementation above. They did not run the task to completion and do not count as fresh-agent passes. A newly prepared second run stopped at preflight because the desktop was locked while AX/Screen Recording permission remained available. No fixture or fresh Agent launched; its separate report is **0 passed, 0 failed, 1 blocked, 7 unverified**. The original failed attempt is not upgraded by the correction or prerequisite record.

An offline regression also reproduced a dev-broker output escape through `window snapshot --output=...`. Test-only commit `6938815` failed two subcases; fix `9109ac5` rejects both attached and separate output options before dispatch. The affected scope/transport suite then passed 17 tests. Production Cueward input and Rust sources are unchanged in this work.

## New document and cross-app tasks

Both tasks use the same entry point, and their logic is in `document_agent_task.py`. They share one prepared run, and `new_document` must run first. Use a fresh Agent and socket directory for each:

```sh
python3 -B dev/computer-use/native-agent-task.py \
  --cli "$PWD/target/release/cueward" --run "$run" \
  --task new_document --socket "$socketdir/a.sock"
```

### `new_document`

The destination must not exist yet. The runner launches the [NSDocument fixture](document-fixture.swift) and moves only that window to an existing inactive Space, the way [first-save-probe.py](first-save-probe.py) does. Setup never creates or switches a Space. It stops if no inactive Space exists, if the move reports a foreground or visible-Space change, or if the window did not land on the chosen Space. A stopped setup serves no Agent and records `attempted: false` with `fixture_unavailable`. The Agent does all editing and saving.

The fixture takes its destination as positional arguments, so it runs as a bare executable. Inside a bundle, AppKit treats those paths as open requests ([initialization contrasts](first-save-initialization.md)). A bare executable has no bundle to unregister. The only change to the fixture is the read-only `observation_sequence` field.

Evidence holds `target_space`, `save_requests`, `contents`, `file_created`, `panel_window_id` and the fresh final receiver fields. The evaluator reads the artifact itself. A run with no artifact still records a complete attempt, with `failure_category: missing_artifact`.

While [#33](https://github.com/Termdock-dev/cueward/issues/33) stays open, expect Save to be disabled in the first panel, so the task ends failed with no artifact. That is the correct result. Do not move the window to a visible Space, activate anything or force the control. One `app inspect` call returns at most 500 nodes and this panel exposes more, so an Agent has to walk subtrees with `--root`, inside the 64-request budget.

The broker checks a target's `app.pid`, which is the app that was inspected. A panel control may report another `receiver_pid`, but its target still names the owned app, so it passes. A target issued for any other app is rejected. The 2026-11-07 run F reached a real panel through the same token check.

### `cross_app`

`cross_app` depends on a passed `new_document` in the same run. Before it reserves or launches anything, the runner evaluates `native-agent-evidence.json`. Without a passed `new_document` it prints why and exits 1. It writes no record, so `cross_app` stays `unverified` with reason `not run`. It can still run later in that run if `new_document` passes. A failed `new_document` cannot be repeated in the same run, so that needs a new prepared run.

`producer_pid` comes only from the `new_document` observer in that evidence. The evaluator also leaves an attempted `cross_app` `unverified` when `new_document` did not pass, for evidence that did not come from this runner.

The [reader](document-reader-fixture.swift) is a separate owned bundle. It starts with nothing open and shows the file only after the Agent presses Open. It has no document class, no editable view and no save path, so its only output is its private state file. Evidence holds `file`, `content`, `read_requests`, `error`, `producer_pid` and the fresh final receiver fields. The evaluator still compares the disk bytes.

## WebView pointer tasks

`--task webview_first_click`, `--task canvas_drag` and `--task canvas_interrupted` use the run and hand-off steps above. Each task needs a separate fresh Agent, receiver and socket directory. Setup reserves `operator/<task>/` once, so rerunning a task needs another prepared run. These tasks have no artifact, so `owned-scope.json` holds only the task, family and goal text from the goal record.

The receiver is `webview-fixture.swift` loading the unchanged `webview-fixture.html`. It is a bare binary, so LaunchServices has nothing to unregister and `cleanup.json` has no bundle entry. It has no first-mouse, hit-testing or event override.

The runner reads `initial` from the receiver before it writes `owned-scope.json`. A page that is not fresh stops the task before any Agent gets scope, and the task is recorded as blocked. Pointer hover adds page events, so a physical pointer over the new window blocks a canvas task. It does not block `webview_first_click`, whose contract checks only the counter.

The broker permits `window snapshot`, `window inspect`, `app inspect` and one pointer action: `window click --count 1` for `webview_first_click`, `window drag` for the canvas tasks. It refuses `app press`, `app set-value` and the other pointer action before dispatch, because each could change the page without the task's counted dispatch. `dispatch_count` is the sum of `AgentSession.dispatches`, never an Agent claim. When the session ends, the runner deletes the snapshot images the CLI reported for the owned window.

After the observer closes, the runner waits three seconds for late page effects, because the [context investigation](webview-context.md) measured effects up to 906 ms after the CLI returned. It then requires a new receiver snapshot and page state newer than the closed observer. A page that stopped reporting fails the final read, and the attempt keeps its receipts and trace without a receiver record.

Only `canvas_interrupted` launches the fixture with its `interrupt` argument. The other two receivers have no added event monitor. In this mode the fixture changes its title on the first tagged `leftMouseDragged` in its own window. Click preparation sends only a down and an up on the title bar, and a click sends no dragged event, so neither can fire it. Production's next check reports `window changed`, stops, releases at the last delivered point and returns `partially_sent`.

`controlled_interruption` is true only when the receiver reports `interruption_triggered` as false in `initial` and true in the final state. The runner never assumes it. A drag too short for production to see the new title returns `sent_unverified`, and the interruption check stays unverified.

`canvas_drag` is expected to fail its displacement check. The page moves the object only while `event.buttons` holds the left button, and the [recorded production drag](background-first-click.md#results) delivered `buttons` 0. That failure is the honest result. Do not change the page or the check to pass it. `canvas_interrupted` needs no displacement, but it still needs the paired release.

The evidence keeps the fixture's `active` and `activations` values, and the evaluator does not check them for these tasks. Click preparation tells the target process it is active, so the fixture's own counter can grow with each dispatch. The desktop observer's `target_active` samples and activation events still apply.

No fresh Agent has run these tasks. Offline tests manufacture evidence. A smoke reached `ready` for each task and finished without sending input. A separate in-process check confirmed the title trigger with synthetic events: the title-bar pair, a real down and an untagged drag did not fire it, and a tagged drag did. Delivery through the production `window drag` route in this fixture is unverified until an operator run.

## Recorded attempts: 2026-11-07

Each row used a separate fresh Agent and receiver with Cueward 0.6.1. Another project's test app ran on the same desktop during these runs. The [aggregate](results/2026-11-07-native-agent.json) keeps the details.

| Run / task | Broker requests | Independent result | Full acceptance |
| --- | ---: | --- | --- |
| C / existing document | 13 | Exact artifact; zero activations | Failed: foreground changed without physical input |
| D / existing document | 14 | Exact artifact; zero activations; maximum sample gap 79.1 ms | **Passed** |
| E / window dialog | 6 | Stopped by the harness when the Save panel opened | Failed: harness rejected the panel |
| F / window dialog | 26 | Exact artifact; native save and bystander checks passed; zero activations | Failed: foreground changed without physical input |

Each run reports its own totals, and they are not combined.

Run C's change came from `folio-e2e`, which started 26 seconds into the run with no physical input in the second before. In run F no change moved to the receiver, and two of the three changes happened while no Cueward command ran. The third overlapped a read-only inspection. `folio-e2e` restarted during that window, but the other process IDs were gone before they could be named, so the source is not proven. Both runs stay failed: the check does not excuse a change, whoever made it.

Run E exposed a wrong assumption in the harness. The sheet is the panel window, but its content sits in a second native window, and the Save panel service's nodes belong to that second window. The binding required the panel's own ID, so it could never hold on a real panel. A service node now binds to the panel ID or to a window that the host exposes inside the same sheet. The first attempt also failed before launch: the fixture standardized `/private/tmp` to `/tmp`, while the runner compares resolved paths. The fixture now reports the resolved path.

`window_dialog` cannot pass yet even on a quiet desktop. As described above, the observer starts with the host alone, so auxiliary service coverage stays unverified.

A load average near 20 did not predict sample gaps: a 10-second probe at that load had a maximum gap of 17.2 ms. Check for other apps taking the foreground instead.

In run F the Agent wrote one inspection result to a file outside the workspace, against its handoff rules, then deleted it and reported it. It did not touch the artifact or the app.

## Recorded attempts: 2026-11-06

Each row used a separate fresh Agent and receiver, with the observer's input attribution and the unchanged fixture. A person used the computer during the runs. The Agent handoff held only the goal, the owned PID and window, the broker command form and the generic app documentation.

| Run / task | Broker requests | CLI commands | Independent result | Full acceptance |
| --- | ---: | ---: | --- | --- |
| A / existing document | 18 | 17 | Exact artifact; bystander and workspace preserved | Failed: receiver activation |
| B / existing document | 18 | 17 | Exact artifact; zero activations; all other checks passed | Unverified: observer gap of 199.5 ms |
| B / calculation | 14 | 13 | Exact `69.85` plus newline; zero activations; 1,458 pointer changes attributed to physical input | **Passed** |

Run A totals are **0 passed, 1 failed, 7 unverified**. Run B totals are **1 passed, 0 failed, 7 unverified**. They are separate runs and are not combined.

Run A's activation came from a person, not from Cueward. A physical input arrived 13 ms before the activation, the pointer was inside the receiver window, and no command was running. It stays failed: the check does not excuse receiver activation, and a person touching the receiver means the run cannot show isolation.

Run B's three largest existing-document gaps fell in the first two seconds, while load averages were 14 to 16 from unrelated work. It stays unverified. Sampling cannot show coverage across a gap above 100 ms.

In the calculation run the Agent wrote one AX inspection result to a file outside the task workspace, against its handoff rules, and reported it. The file did not touch the artifact or the app, so no check changed. As before, this runner covers the owned document fixture and cannot demonstrate Calculator UI operation.

Both Agents reported that AX exposes no saved state: Save returns `sent_unverified`, and the window shows no change. One Agent noted that `--root menu` returned `app AX root disappeared` for an app that was still running, which reads as if the app had gone.

## Recorded attempts: 2026-10-04

Each row used a separate fresh Agent and receiver. Post-fix attempts used the unchanged fixture and the corrected broker trust boundary. Command counts include dispatched CLI errors, while request counts also include rejected broker requests and finish.

| Version / task | Broker requests | CLI commands | Independent result | Full acceptance |
| --- | ---: | ---: | --- | --- |
| Pre-fix existing document | 13 | 11 | Opened once; no edit, Save or artifact match | Failed: artifact and interference |
| Post-fix existing document | 11 | 10 | Opened once, saved once; exact Unicode edit, original/bystander hashes and workspace inventory preserved | Failed: interference |
| Post-fix calculation | 18 | 16 | Opened blank file once, saved once; exact independently checked `69.85` plus newline | Failed: interference |

Pre-fix totals are **0 passed, 1 failed, 7 unverified**. The separate post-fix run totals are **0 passed, 2 failed, 6 unverified**. Historical [2026-10-03 failures](agent-session.md#recorded-task-attempt-2026-10-03) remain unchanged, not combined or converted into passes.

After these recordings, four additional offline regressions exposed and corrected the runner's baseline, native IO, incomplete-session and cleanup failure paths. At that point, rechecking the retained post-fix observations produced the unchanged failed-task report without desktop actions. Older supplied identity/artifact-only evidence keeps its documented contract; the evaluator does not authenticate arbitrary manually supplied evidence.

PR #71 review follow-up reproduced false passing when final activation occurred after the desktop interval. Six additional offline regression methods cover late/transient activation, malformed supplemental state, fresh positive observations, frozen/rebound snapshots and runner ordering. They failed before the fix and pass afterward; the full Python suite now passes 154 tests. The changed fixture compiles under strict Swift 6 with warnings as errors, without launching it. Production Rust/input code is unchanged, so the prior release/Rust checks remain applicable. No new fresh-agent desktop tasks were run. Historical recordings lack the new sequence acknowledgement and are not upgraded to fresh-final-snapshot coverage; their failed totals and raw evidence remain unchanged.

All three receivers stayed inactive with zero owned activation events; visible Space maps stayed stable across three observed displays. Maximum sampling gaps were 42.4, 43.8 and 64.2 ms. Foreground and pointer changes were retained without attribution or filtering. Transport intervals include Agent/start waiting, not just inference time. Sampling cannot exclude shorter transients, and disk success does not override isolation failure. All owned receiver/observer executable paths were absent after cleanup; no user windows or files were selected.

Accepted first save, cross-app artifact consumption, completed window/dialog acceptance, accepted canvas drag/interruption, unintegrated real apps and controlled physical-input/lock coverage remain outstanding. These synthetic native tasks do not satisfy the whole issue.
