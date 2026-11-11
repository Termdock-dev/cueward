# First-save creation-context diagnostic

This extends #33 preparation with the read-only [desktop observer](desktop-observer.md). It is not a fresh-agent run, real-app acceptance or a product workaround. The standard NSDocument fixture's Save panel is unchanged. Only its attached sheet's native window ID is additionally observed.

```sh
python3 -B dev/computer-use/first-save-context-probe.py \
  --cli /absolute/path/to/cueward --context inactive --output new-local-report.json
```

Use a new output path for each trial. `visible` creates the first Save panel with the owned document on a visible Space; `inactive` moves that document to an existing inactive Space before its first Save request; `warm` creates the panel while visible and then moves the document. The last case is only a contrast. It does not satisfy a task requiring first Save entirely on an inactive Space. No Space is created, switched or deleted.

The runner observes current AX app roots, edits the synthetic text, requests document Save once and reads the panel. AppKit may expose the same sheet as both a parent-window child and a separate app root. When separate sheet roots are observed, the diagnostic selects controls within those roots rather than deduplicating by labels/bounds. Multiple observed sheets or Save controls remain ambiguous. AX text controls may omit AXEnabled; an issued actionable target is allowed unless explicitly disabled. Panel Save still requires exactly one observed enabled control and current target.

The warm case reinspects all roots after moving; it never reuses a pre-move panel target. Before dispatching panel Save, the runner independently reads the document and attached panel's native Space memberships. In inactive cases both must match the requested inactive Space. It never forces a disabled control, activates a receiver or retries uncertain input/Save. Successful CLI delivery is not file completion: exact saved UTF-8 bytes are checked separately. Cross-app reading is not included in this diagnostic.

The desktop observer covers the marked execution and command boundaries. Raw AX observations, receipts, receiver state and trace remain in the selected local outputs. Receiver writers terminate before their temporary directories are removed; evidence survives cleanup and errors. Existing outputs are not overwritten. Deterministic tests check command receipt/error retention, sheet selection, disabled/ambiguous controls, current-target reuse prevention, Space membership, file verification and cleanup ordering.

## macOS 27.0.1 comparison

The [aggregate result](results/2026-10-03-first-save-context.json) records one controlled trial per context on build 26A434 with three online displays:

| Panel creation context | Save enabled | Exact file created | Stationary observation check |
| --- | --- | --- | --- |
| Document already inactive | No | No; no panel Save dispatched | Passed at sampled resolution |
| Document visible | Yes | Yes | Failed: observed pointer movement |
| Visible, then document and panel moved inactive | Yes before and after move | Yes | Failed: observed pointer movement |

Foreground PID, receiver activation and all three visible-Space mappings remained unchanged at the observed resolution in each run. Maximum execution sample gaps were 59.6 ms (inactive), 119.2 ms (visible) and 128.8 ms (warm). Only the inactive run met the 100 ms coverage bound. The visible/warm runs exceeded that bound and recorded unattributed pointer movement; no no-interference pass is claimed for them. Both warm document/panel memberships were independently confirmed in the inactive Space before panel Save.

This reproduces the creation-context distinction on the current build. It supports investigation of native Save-panel initialization, but does not establish an AppKit root cause or general app compatibility. #33 remains open. Inactive-first controls must not be bypassed with activation or a warm-up fallback.

## Initialization and real-app follow-up

The [2026-10-04 initialization contrasts](first-save-initialization.md) add complete-bundle and launch-delegate controls plus an unmodified TextEdit artifact comparison. They do not establish a product defect, repair the disabled panel or replace these historical results with an acceptance pass.

## Target-only activation: 2026-11-07

Click preparation raised one more possible cause: the Save control might stay disabled only because the app is inactive. The [aggregate](results/2026-11-07-first-save-activation.json) records the test.

On an inactive Space, the unchanged fixture opened its first Save panel with Save disabled. Cueward's target-only activation record was then sent, first for the document window and then for the attached panel window. The panel window belongs to the document process. Both times the app reported itself active, and Save was still disabled after 200 ms and after 1,200 ms. No artifact was created.

Setting the filename field through `app set-value`, with the same name and with a changed name, was confirmed and also left Save disabled after 600 ms. New Folder is disabled at the same time, while Cancel stays enabled.

Activation of the target alone does not explain the disabled control, so the click preparation route does not solve #33. Foreground activation and forced panel input remain outside this work.

One `app inspect` call returns at most 500 nodes, and this panel exposed 1,952. The unmodified [first-save probe](first-save-probe.py) now stops before Save and reports `save_control_unavailable` with `panel_traversal_truncated: true`. Walking subtrees with `--root` reaches the control. An agent has to do the same.

## Diagnostic continuation: 2026-11-12

The unreleased [bounded subtree continuation](first-save.md#bounded-subtree-continuation) now replaces the two independent one-read-per-window implementations. The context comparison still records every actual CLI receipt through `ObservedCLI`. Continuation only reads observed refs, retains coverage limits and refuses changed public app/root/node observations. It does not enable, press or replay a disabled Save control, change the observer's sampling threshold or introduce a product fallback.

Both Python entry points recover Save beyond 500 nodes in offline native-boundary regressions. The initial live preflight was locked. After the operator unlocked the desktop, one new run on macOS 27.0.1 arm64 produced the [sanitized aggregate](results/2026-11-12-first-save-traversal.json):

| Observation | Result |
| --- | --- |
| Retained panel nodes | 1,028; traversal still incomplete |
| Panel Save | One observed control, disabled, no action target |
| Save requests | One document request, zero panel Save dispatches |
| Artifact | Not created |
| Owned receiver | Inactive, zero recorded activations |
| Stationary desktop observation | Passed at sampled resolution; maximum gap 41.95 ms within the 100 ms bound |
| Cleanup | Receiver and owned observer processes absent; context temporary directory absent |

The marked execution lasted 15.27 seconds; compilation/preparation and cleanup are outside it. Raw targets, paths, process IDs and desktop records remain private. Source hashes in the aggregate identify the diagnostic code used. This confirms useful control discovery without forcing Save, not full panel traversal, a repair of disabled Save, fresh-agent/real-app acceptance or completion of #33. No action was replayed, and #33 remains open.
