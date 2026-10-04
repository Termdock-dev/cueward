# Fresh native document task execution

This continues [#36](https://github.com/Termdock-dev/cueward/issues/36) with runnable owned `existing_document`, `calculation` and `window_dialog` tasks. The [document/calculation aggregate](results/2026-10-04-native-agent.json) records successful post-fix disk results, but **zero full task passes** because foreground and pointer observations changed. The [window/dialog aggregate](results/2026-10-05-window-dialog-agent.json) retains a failed initial attempt and a locked-session prerequisite check. #36 remains incomplete; #33 is not repaired by this change.

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

Cross-app artifact consumption, completed window/dialog acceptance, accepted canvas drag/interruption, unintegrated real apps and controlled physical-input/lock coverage remain outstanding. These synthetic native tasks do not satisfy the whole issue.
