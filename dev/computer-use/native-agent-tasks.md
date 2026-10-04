# Fresh native document task execution

This continues [#36](https://github.com/Termdock-dev/cueward/issues/36) with runnable owned `existing_document` and `calculation` tasks. The [aggregate](results/2026-10-04-native-agent.json) records successful post-fix disk results, but **zero full task passes** because foreground and pointer observations changed. #36 remains incomplete; #33 is not repaired by this change.

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

Cross-app artifact consumption, changing native dialogs/multiple windows, accepted canvas drag/interruption, unintegrated real apps and controlled physical-input/lock coverage remain outstanding. These two synthetic native tasks do not satisfy the whole issue.
