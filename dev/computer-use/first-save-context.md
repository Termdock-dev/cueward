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
