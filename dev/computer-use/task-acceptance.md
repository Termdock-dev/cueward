# Offline task acceptance framework

This is the preparation slice of [#36](https://github.com/Termdock-dev/cueward/issues/36). It creates synthetic goals and independently checks saved bytes and supplied receiver observations. It does not launch apps, drive UI, generate an execution observer, or run the agent. The diagnostic first-save/WebView probes remain separate and do not count as fresh-agent task acceptance.

## Prepare and reset

```sh
python3 dev/computer-use/task-acceptance.py prepare --parent /tmp
```

The command prints the unique run directory. Each invocation creates a new run ID and fresh `work/` directory, preserving previous evidence and reports. `seeds/` contains the unchanged source fixture; `manifest.json` holds the verification baseline. Give the agent only `agent-goals.json`, newly owned permitted app/PID/window scope and the task destination. The goals contain no selectors, AX refs, coordinates or recorded actions. The baseline is an operator oracle, not a security boundary against another process with the same filesystem access.

Prepare a separate new receiver for every task, including the interrupted canvas task. Native window/dialog work needs two owned windows. The operator supplies app instances and an independent observer; cleanup belongs to that setup and must only terminate its newly owned receivers and remove its disposable data. A shared panel service is not newly owned and must not be terminated. This framework deliberately preserves run directories for audit. Reset by preparing a new run, without reusing input targets or old PIDs/windows. Retain or copy any needed reports before manually removing an owned run directory.

The eight tasks cover new and existing documents, cross-app reading, a changing window/dialog, a decimal calculation, ordinary WebView first click, a canvas drag and a separately interrupted drag. The calculation's requested output is exactly `69.85` followed by a newline. File checks read at most 1 MiB and reject symlinks, linked parent directories, hardlinks and nonregular files in this synthetic suite; this restriction is not a general product file-reading contract.

## Evidence contract

Copy `evidence-template.json` to your own evidence file and populate `tasks` by task ID. The top-level `schema` is `1`, `run_id` must match the prepared run, and `kind` is `agent`, `diagnostic` or `simulation`. Only `agent` results can pass acceptance. Regression tests manufacture evidence to test the checker; they are not actual task runs.

For an unattempted task with a missing prerequisite, provide `attempted: false`, `prerequisite` (`session_locked`, `permission_denied`, `missing_app`, `operator_unavailable` or `fixture_unavailable`) and a nonempty `blocked_reason`. Other unattempted or omitted tasks remain `unverified`. A failure after attempting the task does not become blocked by supplying a prerequisite reason.

Every attempted record needs:

- The same `run_id`, its `task_id`, `attempted: true`, and all newly owned `receiver_pids`. Reusing a PID across separate tasks fails freshness checks.
- `receipts`: one entry per tool command, each with `command`, `elapsed_ms` and `status`. Command count and tool time come from this list; retain the original receipts separately. `sent_unverified` alone never verifies an effect. Where an effect cannot identify whether the tool, app or agent caused failure, supply an optional `failure_category`; the report labels it as operator supplied rather than inferring a cause.
- `observer`: an independent receiver record with `source: "receiver_observer"`, matching `run_id`/`task_id`, and an owned `pid`. Additional task fields are described below. The evaluator reads artifacts itself and compares UTF-8 bytes, so an observer's success flag cannot override the wrong disk contents.
- `execution`: `start_ms`, `end_ms` and `samples`, using one monotonic timebase for the entire task execution, excluding setup/cleanup. Samples must cover both boundaries, increase strictly, contain at least three observations and have no gap greater than 100 ms. Each sample includes `ms`, `frontmost_pid`, two-dimensional `pointer`, `visible_spaces` mapping every observed display identity to its visible Space identity, and `target_active` mapping every receiver PID string to a Boolean.

All observed foreground, pointer and visible-Space values must remain equal to the initial baseline, with no receiver active or foreground. New-document tasks also supply `target_space`, which must remain outside the visible Spaces. Missing channels, sparse endpoints or gaps remain unverified. The checker validates the supplied trace's shape and consistency, not observer provenance or whether a caller omitted a display. Sampling cannot exclude transients shorter than its resolution; this report does not claim absolute continuous isolation. A desktop observer covering every connected display is still needed for real runs.

Example attempted new-document record inside the top-level `tasks` object, using synthetic IDs and sample times:

```json
{
  "new_document": {
    "run_id": "COPY-THE-PREPARED-RUN-ID",
    "task_id": "new_document",
    "attempted": true,
    "receiver_pids": [200],
    "target_space": "space-2",
    "receipts": [{"command": "retain the actual command", "elapsed_ms": 20, "status": "sent_unverified"}],
    "observer": {"source": "receiver_observer", "run_id": "COPY-THE-PREPARED-RUN-ID", "task_id": "new_document", "pid": 200, "save_requests": 1},
    "execution": {
      "start_ms": 0,
      "end_ms": 100,
      "samples": [
        {"ms": 0, "frontmost_pid": 100, "pointer": [12, 34], "visible_spaces": {"display-1": "space-1"}, "target_active": {"200": false}},
        {"ms": 50, "frontmost_pid": 100, "pointer": [12, 34], "visible_spaces": {"display-1": "space-1"}, "target_active": {"200": false}},
        {"ms": 100, "frontmost_pid": 100, "pointer": [12, 34], "visible_spaces": {"display-1": "space-1"}, "target_active": {"200": false}}
      ]
    }
  }
}
```

Task-specific observer fields:

| Task | Receiver evidence and independent check |
| --- | --- |
| `new_document` | `save_requests` exactly 1; disk artifact exactly matches supplied content. |
| `existing_document` | Receiver identity plus disk edit, preserved original/bystander hashes, and no extra files in this task workspace. |
| `cross_app` | Observer `file` exactly identifies `work/new_document/result.txt` and `content` matches. Task `producer_pid` must match the passed `new_document` task's observer PID and be distinct from the reader PID. A missing/unverified producer leaves this task unverified. Both reader content and disk bytes are checked. |
| `window_dialog` | Two concrete `owned_windows` plus three ordered `window_observations` for `initial`, `dialog`, `resumed`, with current document/dialog root identities. See the window/dialog contract below. Disk edit and unchanged bystander are checked separately. |
| `calculation` | Receiver identity plus independently calculated exact Decimal result on disk. |
| `webview_first_click` | `dispatch_count: 1`, `initial.clicks: 0`, `final.clicks: 1`; no double click or replay. |
| `canvas_drag` | `dispatch_count: 1`; initial state `object: {x: 100, y: 80}`, `events: []`, `releases: 0`, `dragging: false`; final displacement exactly 80 × 40, `releases: 1`, `dragging: false`. `final.events` contains one trusted `mousedown`, then one trusted `mouseup` with `buttons: 0`. Extra move events are permitted. |
| `canvas_interrupted` | Same fresh initial canvas and paired release checks; `controlled_interruption: true` and a `dispatch_interrupted` receipt. No full displacement is required after interruption; the receiver must release without replay. |

For `existing_document`, `calculation` and `window_dialog`, the [native task runner](native-agent-tasks.md) supplies supplemental final receiver fields: Boolean `active`, nonnegative integer `activations`, positive native `window_id`, positive integer `observation_sequence`, and `fresh_after_sequence` from a baseline read after the desktop interval closes. A valid fresh result has `observation_sequence > fresh_after_sequence > 0`. The `final_receiver_inactive` check fails `active: true` or a positive integer activation count even if the earlier sampled trace passed. Without an established activation, partial, malformed or unfresh supplemental state is unverified. Older records without any final-activity fields retain the identity/artifact-only contract, not a new freshness claim. Native runner output always supplies these fields; this is still supplied-evidence validation, not provenance authentication or proof against sub-sample transients.

### Window/dialog identity contract

The operator's independent receiver observer records two newly owned document windows in `observer.owned_windows`. Each has `pid`, positive native `window_id`, and `file` identifying this run's `target.txt` or `bystander.txt`. Native window IDs must differ and PIDs must be in `receiver_pids`; the target window's PID must match `observer.pid`. Obtain these bindings from the receiver/window observation, not from a window title or AX root index alone.

Each of the three `window_observations` includes:

- `phase`, a distinct nonempty `snapshot_id`, `observed_ms` in the execution timebase, and the exact `target_file`. Times increase strictly within the execution interval.
- `windows`: both document identities above, each extended with its current AX `root`: `{receiver_pid, ref, role}`. Document roots have role `AXWindow` and top-level refs such as `w0`. The observer correlates these roots to native window IDs and retains the underlying observations locally. Native identities stay stable across phases; AX refs may reorder in a fresh snapshot. Two documents cannot share the same `(receiver_pid, ref)` in one observation.
- `active_root`: the root actually inspected/used in this phase, matching the target document root in `initial`/`resumed` and the save-dialog root in `dialog`.
- `dialog`: explicitly `null` in `initial` and `resumed`; in the middle phase it identifies an independently observed save dialog, with `kind: "save"`, `mode`, `owner_window: {pid, window_id}` for the target document, and its AX `root`.

For a same-receiver sheet, use `mode: "sheet"`, role `AXSheet`, and `parent_root` matching the target's current root. Its AX ref must be a descendant of that root. For a standalone save panel, use `mode: "window"`, a distinct positive native `window_id`, and a separate top-level root with role `AXWindow` or `AXDialog`. A dialog cannot reuse either document root. Obtain the save purpose and owning document association from independent receiver observations; a changed label or arbitrary generic modal is not save evidence.

Example middle-phase observation (synthetic IDs; the matching owned inventory and other two phases are also required):

```json
{
  "phase": "dialog", "snapshot_id": "fresh-dialog", "observed_ms": 500,
  "target_file": "/RUN/work/window_dialog/target.txt",
  "windows": [
    {"pid": 203, "window_id": 501, "file": "/RUN/work/window_dialog/target.txt",
     "root": {"receiver_pid": 203, "ref": "w0", "role": "AXWindow"}},
    {"pid": 203, "window_id": 502, "file": "/RUN/work/window_dialog/bystander.txt",
     "root": {"receiver_pid": 203, "ref": "w1", "role": "AXWindow"}}
  ],
  "active_root": {"receiver_pid": 203, "ref": "w0.0", "role": "AXSheet"},
  "dialog": {
    "kind": "save", "mode": "sheet", "owner_window": {"pid": 203, "window_id": 501},
    "root": {"receiver_pid": 203, "ref": "w0.0", "role": "AXSheet"},
    "parent_root": {"receiver_pid": 203, "ref": "w0", "role": "AXWindow"}
  }
}
```

Missing window/root/time fields, including older snapshot-label-only records, are `unverified`. Contradictory identities, no dialog in the dialog phase, an unrelated owner/parent, or a dialog still present when resuming are `failed`. This validates supplied relationships; the framework still does not collect observations or authenticate their provenance. Product app/window APIs are unchanged.

### Shared save-panel receivers

A save sheet or its descendants may use a shared service's actual AX PID. Document roots must still belong to the newly owned host. Optional `observer.auxiliary_receivers` entries authorize only the independently observed native panel, not arbitrary windows of the service:

```json
{"pid": 204, "window_id": 503, "owner_window": {"pid": 203, "window_id": 501},
 "source": "native_ax_window_binding"}
```

The matching dialog must supply `window_id` and `native_binding` with the same source, panel ID and canonical owner, `owner_window_cf_equal: true`, and distinct positive `receiver_pids` independently read from the native-bound sheet root/descendants. The dialog root's actual PID must be in that list; all nonhost PIDs must exactly match the auxiliary entries. The current native observer uses direct `_AXUIElementGetWindow` plus sheet `AXWindow` equality with the owner, not a service name, label, AX index or decoded action token as proof. A service-owned sheet retains the same parent/descendant relationship as a host-owned sheet. A standalone auxiliary panel's owner association remains unverified.

Auxiliary PIDs must not enter the task's newly owned `receiver_pids` or satisfy fresh-receiver checks. Unknown/duplicate/mismatched services, owners or native identities fail; absent required proof remains unverified. Any supplied `native_binding` is validated, including host-only proof; contradictions cannot be ignored because there are no auxiliary entries. Legacy records without an auxiliary contract or native proof retain the same-host rules.

`auxiliary_receiver_trace` requires the execution observer to cover the host and every bound service from task start through task end. Every sample needs Boolean service `target_active`, with at least three monotonically increasing samples and gaps at most 100 ms. Service foreground/activity or activation events fail; missing earlier coverage stays unverified. Only complete validated auxiliary coverage permits a temporary trace scope containing host plus services; it does not mutate ownership, drop samples or filter foreground/pointer/Space changes. The current runner discovers the service after the host-only desktop observer starts, so it cannot claim this full auxiliary coverage.

Native runner records also include `native_io_complete`, checked separately from disk bytes. A matching artifact cannot compensate for incomplete normal target save/write or an edited/saved bystander. These producer checks do not authenticate arbitrary supplied evidence.

Failure categories accepted from the operator are `prerequisite_unavailable`, `tool_observation_failure`, `tool_dispatch_failure`, `application_rejection`, `partial_observation`, `stale_target`, `missing_artifact`, `interference_observed`, `agent_decision_error`, and `unknown`. Raw personal desktop data stays local; publish only reviewed synthetic/aggregate evidence.

## Evaluate

```sh
python3 dev/computer-use/task-acceptance.py evaluate \
  --run /tmp/cueward-acceptance-RUN \
  --evidence /tmp/cueward-acceptance-RUN/evidence.json \
  --output /tmp/cueward-acceptance-RUN/report.json
python3 -m unittest discover -s dev/computer-use -p 'test_*.py' -v
```

JSON inputs are limited to 64 MiB. Reports include all eight tasks, per-check results, `passed`/`failed`/`blocked`/`unverified` totals, metrics and separate native AX/WebView support matrices. Exit status is 0 only if every task passes, 1 for any incomplete or unsuccessful task, and 2 for invalid run-level input or output failure. The CLI rejects report paths that overwrite the prepared data, task artifacts or evidence input.

This offline preparation layer does not execute fresh-agent tasks. The separate [native task runner](native-agent-tasks.md) supplies owned execution for existing-document editing, calculation and changing windows/save sheets. Existing-document/calculation post-fix artifacts passed, but full interference checks failed; initial window/dialog execution failed and its replacement run was blocked before launch. Importing a probe's completed/recorded status does not establish a passed task. Real app coverage and operator-assisted experiments remain outstanding; #36 stays open.
