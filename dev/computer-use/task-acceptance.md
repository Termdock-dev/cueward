# Offline task acceptance framework

This is the preparation slice of [#36](https://github.com/Termdock-dev/cueward/issues/36). It creates synthetic goals and independently checks saved bytes and supplied receiver observations. It does not launch apps, drive UI, generate an execution observer, or run the agent. The diagnostic first-save/WebView probes remain separate and do not count as fresh-agent task acceptance.

## Prepare and reset

```sh
python3 dev/computer-use/task-acceptance.py prepare --parent /tmp
```

The command prints the unique run directory. Each invocation creates a new run ID and fresh `work/` directory, preserving previous evidence and reports. `seeds/` contains the unchanged source fixture; `manifest.json` holds the verification baseline. Give the agent only `agent-goals.json`, newly owned permitted app/PID/window scope and the task destination. The goals contain no selectors, AX refs, coordinates or recorded actions. The baseline is an operator oracle, not a security boundary against another process with the same filesystem access.

Prepare a separate new receiver for every task, including the interrupted canvas task. Native window/dialog work needs two owned windows. The operator supplies app instances and an independent observer; cleanup belongs to that setup and must only terminate its recorded receivers and remove its disposable data. This framework deliberately preserves run directories for audit. Reset by preparing a new run, without reusing input targets or old PIDs/windows. Retain or copy any needed reports before manually removing an owned run directory.

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
| `window_dialog` | `window_observations` for phases `initial`, `dialog`, `resumed`; three distinct nonempty `snapshot_id` values and each `target_file` identifies this task's `target.txt`. Disk edit and unchanged bystander are checked separately. Preserve actual observations alongside these identities. |
| `calculation` | Receiver identity plus independently calculated exact Decimal result on disk. |
| `webview_first_click` | `dispatch_count: 1`, `initial.clicks: 0`, `final.clicks: 1`; no double click or replay. |
| `canvas_drag` | `dispatch_count: 1`; initial state `object: {x: 100, y: 80}`, `events: []`, `releases: 0`, `dragging: false`; final displacement exactly 80 × 40, `releases: 1`, `dragging: false`. `final.events` contains one trusted `mousedown`, then one trusted `mouseup` with `buttons: 0`. Extra move events are permitted. |
| `canvas_interrupted` | Same fresh initial canvas and paired release checks; `controlled_interruption: true` and a `dispatch_interrupted` receipt. No full displacement is required after interruption; the receiver must release without replay. |

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

No real fresh-agent runs are recorded by this change. Existing probes lack the complete multi-display execution trace expected here, so importing their completed/recorded statuses does not establish a passed task. Real app coverage and operator-assisted #31/#34 experiments remain outstanding; #36 stays open.
