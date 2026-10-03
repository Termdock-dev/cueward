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

- Markers and samples use one observer `systemUptime` timebase. Each marker is bracketed by immediate samples; caller wall clocks are not substituted. The evaluator checks actual gaps against its existing 100 ms maximum, not the requested timer interval.
- Each sample includes `sample_end_ms` so the desktop-read duration is retained. Channels are read sequentially, not as one atomic desktop snapshot.
- Every sample correlates CoreGraphics online display identities with the dynamically discovered read-only `SLSCopyManagedDisplaySpaces` catalog. Missing, duplicate or uncorrelated identities produce an observation error. Mirrored displays or shared-Space configurations without a per-online-display mapping remain incomplete rather than being silently skipped or inferred.
- Foreground PID, pointer coordinates, receiver active state and lock state are sampled. Workspace activation notifications also record transient receiver activations between samples. Events, errors and lock observations are checked by the acceptance evaluator when native-observer metadata is supplied. Provenance is not authenticated; sub-sample transitions and notification delivery limits remain explicit.
- Receiver processes must exist for the whole marked interval. A receiver exit is an observation error. Use separate observers/task intervals for later-created reader processes; do not fabricate earlier samples for a PID that was not being observed.
- The default timer interval is 20 ms, configurable from 5 to 50 ms. The default helper deadline is 10 minutes, at most 30 minutes. Caller exit, stdin EOF or an explicit stop ends the helper. The Python context cleans up only its owned helper, preserves evidence, and limits retained output to 64 MiB. Abrupt termination can leave a truncated record.
- `stationary` checks pointer constancy for idle-user trials. `physical_concurrent` retains actual pointer data but returns `unverified` until independent physical-input/receiver attribution exists. It neither rejects user movement as automation interference nor asserts that it proves no pointer warp. #31 is not completed by this policy.
- This observer can read a lock-state transition but does not add locked-session operation support or persistent AX/ScreenCaptureKit resources. Existing product guards are unchanged.

## Verification

```sh
python3 -B -m unittest discover -s dev/computer-use -p 'test_*.py' -v
```

The tests execute the actual Foundation display-correlation/configuration model and native control/run-loop protocol with the desktop-read boundary substituted. Python tests check boundary coverage, activation/error/lock rejection, scope matching, physical-movement limits, exclusive logs and real subprocess cleanup. These deterministic tests are not desktop compatibility or fresh-agent acceptance. An actual multi-display desktop smoke run remains a separate check.

An unlocked read-only smoke on 2026-10-03 collected 54 execution samples across three online displays with no observation errors. The maximum whole-trace gap was 25.3 ms. Receiver activation, foreground PID and visible Spaces remained unchanged at the observed resolution. Pointer movement was observed, so the stationary-pointer check failed rather than claiming an isolation pass. The [aggregate report](results/2026-10-03-observer.json) contains no raw process/display identities or local paths; full task and physical-input acceptance remain outstanding.
