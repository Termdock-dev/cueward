# Read-only desktop execution observer

This is the observation slice of #36 and shared preparation for #31/#33. It is not an agent task runner or a claim that those issues are complete. The standalone helper posts no input, takes no screenshots, activates no apps, and does not enumerate window titles or read keyboard/text contents. No new package or production service is introduced.

Use only explicitly owned, already running receiver PIDs. Compile before the task, then keep the observer alive while CLI calls or agent decisions run:

```python
from pathlib import Path
import tempfile
from desktop_observer import compile_observer, DesktopObserver, observation_checks

with tempfile.TemporaryDirectory(prefix="cueward-observer-build-") as build:
    binary = compile_observer(build)
    with DesktopObserver(binary, owned_pids, Path("unique-local-trace.jsonl")) as observer:
        start = observer.mark("task-start")
        # Run the task here. Do not provide fixed steps to a fresh-agent acceptance run.
        end = observer.mark("task-end")
        execution = observer.execution(start, end)
        checks = observation_checks(execution, owned_pids)
```

Put `execution` into the task evidence alongside actual receipts and independent receiver/artifact evidence. Raw JSONL includes readiness, samples, activation events, errors, markers and termination reason. Existing evidence is never overwritten. Keep raw data local and publish only reviewed synthetic/aggregate results.

## Coverage and limits

- Markers and samples use one observer `systemUptime` timebase. Readiness also includes `system_uptime_origin` for correlation with independently observed receiver event uptimes. Each marker is bracketed by immediate samples; caller wall clocks are not substituted. The evaluator checks actual gaps against its existing 100 ms maximum, not the requested timer interval.
- Each sample includes `sample_end_ms` so the desktop-read duration is retained. Channels are read sequentially, not as one atomic desktop snapshot.
- Every sample correlates CoreGraphics online display identities with the dynamically discovered read-only `SLSCopyManagedDisplaySpaces` catalog. Missing, duplicate or uncorrelated identities produce an observation error. Mirrored displays or shared-Space configurations without a per-online-display mapping remain incomplete rather than being silently skipped or inferred.
- Foreground PID, pointer coordinates, receiver active state and lock state are sampled. Workspace activation notifications also record transient receiver activations between samples. Events, errors and lock observations are checked by the acceptance evaluator when native-observer metadata is supplied. Provenance is not authenticated; sub-sample transitions and notification delivery limits remain explicit.
- Receiver processes must exist for the whole marked interval. A receiver exit is an observation error. Use separate observers/task intervals for later-created reader processes; do not fabricate earlier samples for a PID that was not being observed.
- `panel_services=True` adds one exception, for `com.apple.appkit.xpc.openAndSavePanelService` only. Only the `window_dialog` runner turns it on. Each sample reads the kernel process list (`KERN_PROC_ALL`) before its timestamp. The kernel keeps only the first 16 bytes of a name, so the full path confirms each candidate. These reads never signal, attach to or message a service.
- The sample where a service first appears carries a `panel_services` record with `pid`, `executable`, `start_ms` and `first_seen_ms`, and `execution()` collects these into `execution.panel_services`. From that sample on, `target_active` has the service's PID. A service that disappears loses its entry. That is not an observation error, because another client's service can exit at any time. Other clients' services appear too, and the evaluator reads only the bound one.
- `start_ms` comes from `ri_proc_start_abstime`, a `mach_absolute_time` value. `systemUptime` counts the same ticks, so the conversion needs only the timebase and `system_uptime_origin`. Sleep and wall-clock changes cannot shift it. A service older than the observer has a negative `start_ms`. The listing runs before the timestamp, so a start time never follows its first-seen sample.
- launchd starts the service as `xpcproxy`, which then becomes the service under the same PID. The start time survives that change, so `start_ms` includes the launch stage. This is why each sample lists every process: a listing limited to new PIDs would miss the service.
- `target_active` for a service reads `NSRunningApplication.isActive`. LaunchServices registers a new service after the kernel lists it, 39.6 ms later in the one run that measured this. Until then the value is `false`, because the process has no activation state. A service has activation policy `prohibited`, so this channel can only catch a change in that behavior.
- The default timer interval is 20 ms, configurable from 5 to 50 ms. The default helper deadline is 10 minutes, at most 30 minutes. Caller exit, stdin EOF or an explicit stop ends the helper. The Python context cleans up only its owned helper, preserves evidence, and limits retained output to 64 MiB. Abrupt termination can leave a truncated record.
- When a listen-only session tap is available, each sample carries cumulative `input` counts by source: `physical_pointer`, `automated_pointer` and `physical_other`. Source PID zero is hardware. Key codes and characters are never read. Idle-time APIs are not used because posted events reset them.
- With complete `input` counts, a person's activity no longer fails a trial. Pointer movement must have physical pointer input within 100 ms, and a foreground or visible-Space change must have physical input within one second. Any automated event on the global pointer stream fails. Receiver foreground or activity fails regardless of input. A pointer warp posts no event, so it fails as movement without physical input.
- `execution.marks` holds every observer marker. With them, a change outside every command interval, plus 1,000 ms after its end, is not held against Cueward. See the [evidence contract](task-acceptance.md#evidence-contract).
- Without complete counts, `stationary` keeps checking pointer constancy for idle-user trials. `physical_concurrent` retains actual pointer data but returns `unverified`. It neither rejects user movement as automation interference nor asserts that it proves no pointer warp. #31 is not completed by this policy.
- This observer can read a lock-state transition but does not add locked-session operation support or persistent AX/ScreenCaptureKit resources. Existing product guards are unchanged.

## Verification

```sh
python3 -B -m unittest discover -s dev/computer-use -p 'test_*.py' -v
```

The tests execute the actual Foundation display-correlation/configuration model and native control/run-loop protocol with the desktop-read boundary substituted. Python tests check boundary coverage, activation/error/lock rejection, scope matching, physical-movement limits, exclusive logs and real subprocess cleanup. The kernel listing and start-time conversion also run against real child processes. These deterministic tests are not desktop compatibility or fresh-agent acceptance. An actual multi-display desktop smoke run remains a separate check. The real Save panel check is in the [native task notes](native-agent-tasks.md#recorded-panel-service-check-2026-11-07).

An unlocked read-only smoke on 2026-10-03 collected 54 execution samples across three online displays with no observation errors. The maximum whole-trace gap was 25.3 ms. Receiver activation, foreground PID and visible Spaces remained unchanged at the observed resolution. Pointer movement was observed, so the stationary-pointer check failed rather than claiming an isolation pass. The [aggregate report](results/2026-10-03-observer.json) contains no raw process/display identities or local paths; full task and physical-input acceptance remain outstanding.
