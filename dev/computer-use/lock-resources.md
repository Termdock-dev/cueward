# Retained resources across screen lock: research preparation

This is the runnable preparation slice of [#34](https://github.com/Termdock-dev/cueward/issues/34), not locked-session product support. It establishes an unlocked baseline, retains AX client handles and a ScreenCaptureKit stream, and records a bounded three-phase experiment when the operator later locks and unlocks the desktop. The [aggregate](results/2026-10-04-lock-resources.json) contains only unlocked results; locked and after-unlock rows remain unverified. #33 is unrelated and unchanged.

## Owned scope and freshness

The runner builds a unique accessory AppKit receiver and retains its child process handle. Only that receiver's native window and fixed diagnostic label/button are used. Its ordinary button records synthetic action counts; its drawing and AX value expose a counter that advances every 500 ms. No first-mouse, focus, activation or input-acceptance behavior is overridden. Receiver activation, changed PID/window/run identity, frozen acknowledgements or a dead process stop the run. No existing user app, window or document is selected or closed.

The receiver redraws its synthetic pattern before atomically writing the independent counter and `systemUptime`. The helper retains pre-experiment AX application/element handles and creates another application handle for each new read. These are new client handles, not proof of a separate private AX connection or transport. Attribute reads return only the synthetic `SEQ:` value, never arbitrary user text.

The retained `SCStream` keeps its initial exact PID/window filter; each new screenshot enumerates a new owned-window match and constructs a new filter for `SCScreenshotManager`. Both use desktop-independent single-window capture, with child windows, menu bar, cursor, click indicators, microphone and audio excluded. Shareable-content enumeration is transient; other entries are not logged. Pixels are decoded in memory into the synthetic counter and discarded, never saved as images. The fixed magic/checksum is not provenance authentication. Apple documents [single-window filtering](https://developer.apple.com/documentation/screencapturekit/sccontentfilter) and [sample-buffer screenshots](https://developer.apple.com/documentation/ScreenCaptureKit/SCScreenshotManager/captureSampleBuffer%28contentFilter%3Aconfiguration%3AcompletionHandler%3A%29).

Each sample brackets observations with the receiver's independent before/after counter and timestamps, using the same Swift `systemUptime` clock. The after record must advance beyond the before sequence and the probe's completion time. A route needs at least two distinct increasing, receiver-correlated values in a consistently locked or unlocked phase before it is `fresh`. API success, callback count, a single value or a repeated frozen frame cannot pass. Stale data needs an advancing receiver oracle; blank, missing, stopped, timeout and error outcomes remain separate. A lock transition inside a sample leaves it unverified. Raw timestamps and probe bounds are preserved privately.

## Unlocked baseline

Requires an unlocked desktop, Accessibility and Screen Recording permission, Swift 6 and a macOS 15-or-newer SDK/runtime for this capture configuration. Only macOS 27.0.1, arm64 was actually tested. The command checks existing permissions; it does not request or change them.

```sh
python3 -B dev/computer-use/lock-resource-probe.py --mode baseline --duration 8
```

To include two explicit owned-button action trials:

```sh
python3 -B dev/computer-use/lock-resource-probe.py \
  --mode baseline --duration 8 --diagnostic-input
```

Input is disabled by default. With the flag, at most one retained and one newly resolved AXPress target is tested per phase, without replay. Both still require a fresh lookup of the same native window, nonforeground ownership, the expected role and an enabled button. A failed fresh prevalidation is labelled `prevalidation_failed`, not evidence that the retained target itself failed. Receipts distinguish requested, actually dispatched, guard-rejected and unknown transport outcomes. The receiver independently records each press's time and lock state; an effect appearing only after unlock cannot certify a locked-phase effect. This slice does not test CG keyboard/pointer targets, a persistent capture stream created afresh while locked, or generic app input acceptance.

`baseline_ready` means the four observation routes have established freshness, not that cleanup has completed or optional input has passed. Final `baseline_status` also requires recorded cleanup and no runner errors. Inspect the separate action results. Raw logs, checkpoints and source hashes are retained in a new private run directory; repeat by creating a new run, never editing the previous evidence.

## Operator-assisted lock/unlock experiment

Only start this after a usable baseline, with the user prepared to participate:

```sh
python3 -B dev/computer-use/lock-resource-probe.py \
  --mode lock --duration 180 --diagnostic-input
```

1. Leave the owned fixture alone and wait for `baseline_ready` before locking.
2. Lock the desktop yourself, keep it locked for about ten seconds, then unlock. The tool never locks, unlocks or supplies credentials.
3. Leave the fixture alone while it records three after-unlock samples and cleans up. Its phase labels come from observed lock state, not a user-provided label.

The report always retains `before_lock`, `locked` and `after_unlock`. Missing stages, interleaved phases, changed identity or an unusable freshness oracle remain unverified. Lock mode exit 0 means a complete, ordered three-phase recording with successful cleanup; it can include unavailable or stale locked routes and does not mean `lock_status` passed. `product_operation_support` remains unverified even if every research observation is fresh. If no lock/unlock cycle occurs before the deadline, the command exits nonzero, not success based on the unlocked baseline alone.

AX messaging has a one-second process-local timeout. Capture callback waits have three-second gates; these end waiting, not the underlying noncancellable SDK operation. The controller separately bounds startup/responses/shutdown, retains partial native stdout and action requests before later oracle acknowledgement, and preserves compiler logs. A stale/missing independent receiver acknowledgement can stop the experiment; retained API results remain private evidence, never promoted to fresh or erased as an unattempted run.

Duration is 4–480 seconds, default 8 for baseline or 180 for lock. Receiver/helper lifetimes add bounded cleanup margins, at most 510/500 seconds. Partial output has a 64 KiB line bound and total native stdout a 64 MiB bound. Cleanup uses only retained child handles, checks stream shutdown acknowledgement, unregisters only the unique fixture bundle and preserves all run files. A failed SDK stop, nonzero helper exit or bundle-unregister error remains a cleanup failure. Ctrl-C follows the same cleanup path. No service, schedule or new dependency is installed.

## Recorded preparation: 2026-10-04

The first attempt aborted before readiness with a WindowServer initialization assertion; no observations or input were obtained, and its nonzero helper exit remains a failed cleanup record. The helper now initializes AppKit with prohibited activation policy before its read-only session calls. Two separate later unlocked runs each recorded 14 matching, increasing observations on all four routes, two single button effects, zero receiver activations and successful cleanup. The second of those runs includes the finalized failure checkpoints and input-stage metadata. All owned receiver/helper executable paths were absent after cleanup. Raw evidence remains private, including the failed attempt.

These are operator diagnostics, not fresh-agent acceptance, physical-input isolation or a locked-session result. No full foreground/pointer/Space isolation trace is claimed, and no lock/unlock experiment was performed. Formal product lock guards and all crates are unchanged. #34 stays open for the actual three-phase matrix and remaining input/stream-renewal coverage.

Validation includes strict Swift 6 warnings-as-errors compilation, twelve pure BGRA pattern cases and offline tests of freshness, malformed/missing phases, permission codes, partial transport, failure checkpoints, independently timed action effects, mode-specific completion and cleanup. Run the offline suite without launching a desktop app:

```sh
python3 -B -m unittest discover -s dev/computer-use -p 'test_*.py' -v
```
