# Interactive fresh-agent execution transport

This is an execution slice of #36, not a new production adapter. The agent receives task goals, explicitly owned receiver/window IDs, destinations and generic tool documentation. It selects current observations and actions itself. The transport records actual CLI commands/results and desktop samples; it contains no recorded task steps, selectors, coordinates or fixture-specific action choices. The operator independently reads receiver records/artifacts after execution.

## Start an owned task

Prepare the run with [task-acceptance.py](task-acceptance.md), compile and launch a fresh permitted fixture, and retain its PID/native window identity before starting. An inactive-Space task moves only that owned window during setup. No user app/window is selected. Keep fixture state/oracles private to the operator, not the fresh agent. A receiver must remain alive for the observer's whole interval.

```sh
# Use a new private short temporary directory for the socket. macOS limits Unix socket path length.
socketdir=$(mktemp -d /tmp/cwa.XXXXXX)
python3 -B dev/computer-use/agent-session.py \
  --cli /absolute/path/to/cueward --pid OWNED_PID --window-id OWNED_WINDOW \
  --run-id PREPARED_RUN_ID --task-id TASK_ID \
  --socket "$socketdir/a.sock" --output new-local-task.json
```

Terminal process/session IDs are not shared between Agent runtimes. Use the same-machine socket client rather than giving another Agent the operator's terminal session ID. Wait for `ready` before handing over the task. The socket is mode 0600; an existing socket/file is never replaced. This is local command scoping, not authentication against another process with the same account.

The agent sends one self-selected command at a time:

```sh
python3 -B dev/computer-use/agent-command.py --socket /tmp/cwa.UNIQUE/a.sock \
  app inspect --pid OWNED_PID
python3 -B dev/computer-use/agent-command.py --socket /tmp/cwa.UNIQUE/a.sock --finish
```

Shell-quote values and observed targets normally. App inspection/actions must match the owned app; window inspection/snapshot/pointer targets must match both owned PID and window ID. Space mutation, app opening, arbitrary output paths and global input are not permitted by this transport. Native production target freshness/identity checks still run. One click/drag attempt is allowed per session, including an uncertain attempt; a second pointer dispatch cannot repair the effect. An AX error retains its actual receipt and warns not to replay; the agent must observe before any new decision. The task checker separately enforces native Save-once behavior from receiver evidence.

Requests are limited to 64 KiB and 64 requests, including rejected requests and finish. Client responses are capped at 8 MiB. Each CLI invocation has a 45-second timeout. The broker deadline is 800 seconds and observer deadline 900 seconds; no persistent service or package is installed. Stdin mode remains available for a single owner, but the socket mode permits a separate Agent process. Socket reads have finite timeouts. Owned socket files are removed on orderly exit, including budget/deadline exit.

Receipts/results checkpoint after every dispatched command. Finalization records the full observed execution, request count and termination reason, even if the agent does not send finish. Observer failure preserves records without fabricating an execution interval. Raw logs may contain personal desktop metadata and stay local. The operator cleans up only recorded receiver identities and snapshot paths, then removes its empty socket directory. Prepared runs/evidence are preserved.

## Recorded task attempt: 2026-10-03

The [aggregate report](results/2026-10-03-fresh-agent.json) contains three separate fresh-agent synthetic tasks on macOS 27.0.1, build 26A434. Each Agent was instructed to read only the goal and generic tool documentation, not fixture source/state/oracles or prior diagnostics. An independent operator read actual disk bytes or receiver DOM state. This is not an independent code review or an all-app compatibility claim.

| Task | Actual commands | Independent effect | Acceptance |
| --- | ---: | --- | --- |
| Inactive-first new document | 63 | One document Save request; panel Save remained disabled; no artifact | Failed |
| Ordinary WebView first click | 4 | One pointer dispatch, counter remained 0, no DOM mouse events | Failed |
| Ordinary WebView canvas drag | 3 | One pointer dispatch, object unchanged, no DOM down/up or release | Failed |

The native Agent also encountered stale/mismatched target errors, explored panel controls, and reached the 64-request transport budget. Its final execution trace was archived despite the Agent's subsequent finish call finding no socket. Failed or refused actions are included in the command count. These attempts reproduce unsuccessful outcomes, not a product repair or a definitive root-cause diagnosis. The original terminal-sharing transport failure occurred before UI actions and was retained locally, not counted as a successful task.

All eight tasks remain in the denominator: **0 passed, 3 failed, 5 unverified**. Existing-document edit, cross-app reading, window/dialog ownership, calculation and controlled interrupted drag were not run. Execution timing includes waiting for transport/Agent start and is not agent-only inference latency. Sampled pointer movement is unattributed and cannot be removed to manufacture an isolation pass. Physical-user overlap, lock/unlock persistence and real-app coverage remain separate requirements; #31/#32/#33/#34/#36 stay open.

Offline tests exercise scoped commands, no replay after uncertain pointer delivery, receipt retention, finalization on observer failure, and real cross-process socket communication/cleanup. They are not desktop task passes. Rust product sources and input/lock guards are unchanged.
