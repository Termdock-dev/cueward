# Background pointer repair work in progress

No generic sender-side correction has been established for #32. The draft repair PR preserves the failing ordinary-receiver baseline, distinguishing actual effects from dispatch receipts. Production routing, target binding and interruption guards remain unchanged. #31, #32, #33 and #36 are not closed by these diagnostic tools.

## Reproduce the acceptance gap

Build the CLI and run the ordinary receiver with an unlocked desktop and existing Accessibility and Screen Recording access:

```sh
cargo build --release
python3 -B dev/computer-use/webview-context-probe.py \
  --cli /absolute/path/to/cueward --parent /private/local/evidence \
  --variant ordinary --context inactive
```

Use `--context visible` for the visible-background contrast. Each run owns fresh receivers; neither route activates apps, changes the visible Space, posts global input or retries uncertain actions. See [webview-context.md](webview-context.md) for bounds, cleanup and the tested platform. A `recorded` diagnostic is not an effect pass.

## Sender hypotheses checked

The [candidate results](results/2026-10-04-sender-routing.json) record two temporary sender-source variants. Both retained the production private event source, exact target binding, locks, caller checks, interruption and release logic. No Command modifier was added. These variants were not adopted in production:

| Candidate | Change from production | Ordinary receiver result |
| --- | --- | --- |
| CG default subtype | Change pointer subtype from 3 to the documented default 0; leave other routing unchanged | First and subsequent single clicks each reach App as down/up, with no DOM effect within the three-second budget; drag reaches App as 27 events, with no DOM effect |
| NSEvent construction | Construct native mouse events with the existing window identity, local point, monotonic timestamp and click count; extract CGEvent and attach the same private source; retain production routing and flags | Same missing effects and App event counts |

Temporary source copies ran through the existing interpreted helper launched by the actual CLI. All four case intervals passed the sampled stationary desktop check; all owned receivers were stopped. An alternative constructor or documented subtype alone is insufficient on this tested build. Two failed variants do not rule out other routes.

A sender-only field inspection suggested a stale timestamp hypothesis. The independent ordinary receiver instead observed 27 strictly increasing, current timestamps over approximately 618 ms, agreeing with CGEvent timestamps. Its window remained non-key and non-main. This trace does not support shipping a timestamp correction as the first-mouse fix.

A read-only AX contrast found application `AXFocusedWindow`/`AXMainWindow` and window `AXFocused` not settable. Window `AXMain` was already true and settable while native `isKeyWindow` remained false. No setter was called. There is no proven writable AX focus route for this instance; AX main state is not interchangeable with native key state. This is not a claim about all applications.

## Required before a functional fix is ready

1. Establish a sender route that produces exactly one click and the expected drag displacement on an unmodified receiver, without activation, a visible-Space switch, Command spoofing, physical button-state changes or receiver hooks. Isolate native first-mouse policy from DOM held-button state rather than assuming one change fixes both.
2. Keep a failing behavior regression in a test-only commit, then the demonstrated product correction in a separate commit. Verify the same receiver effect after the correction, including interruption and accepted release, plus existing native pointer and keyboard regressions. A patched diagnostic control cannot satisfy this gate.
3. Repeat physical overlap on the corrected native route, then verify applicable WebView cases. Retain the full task denominator and independently check effects; no lost or unrun case becomes a pass. First-save panel creation and retained lock/unlock resources remain separate follow-up work.

No activation/global-input fallback or receiver-specific adapter is proposed by this PR. The repair remains draft until the first gate has evidence.
