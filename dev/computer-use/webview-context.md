# WebView first-mouse and button-state context

The #32 investigation now distinguishes App receipt, native first-mouse policy, WKWebView callbacks, DOM button state and actual effects. No production input route is changed; #32 and the dependent #31 accepted-drag case remain open.

```sh
python3 -B dev/computer-use/webview-context-probe.py \
  --cli /absolute/path/to/cueward --parent /private/local/evidence \
  --variant ordinary --context inactive
```

`ordinary` uses WKWebView directly, with no first-mouse or event-handler override. Its App-level monitor records only mouse events inside its newly owned window and returns each event unchanged. `observed` uses a diagnostic subclass whose first-mouse and pointer handlers log and call `super`; it returns the native first-mouse result. `control` differs from `observed` by returning true for first mouse. **Control effects are never ordinary-app support or a product workaround.** No foreign/global keyboard text or external app/window contents are read. Mouse-event contents are limited to the owned receiver; global pressed-button state is read, never mutated.

Choose `visible` to keep the receiver on the current visible desktop while its app remains in the background. `inactive` moves only the new receiver to an existing inactive user Space and independently verifies membership. Neither mode activates an app, switches/creates a Space, posts global input or alters global button state. Each case has its own bounded desktop observer. Raw evidence is private and retained; cleanup terminates only the owned process handle and removes PID-verified snapshots.

One receiver gets two independently named, preplanned single-click trials, each count one. The second is a first-versus-subsequent diagnostic, not a fallback or a retry counted as first-click success. A separate fresh receiver gets one drag. Errors/partial dispatch stop remaining actions in that receiver; no input is replayed. Coordinates come from current synthetic DOM rectangles and fresh actual snapshots. The native CLI is the only input route.

## Observation correction

A fixed 500 ms post-command read was insufficient: a preliminary control trial reported no second click and an apparently unreleased drag, but the same receiver's later state contained both click effects and the drag release. This does not prove event loss. The new observer waits up to three seconds for a **new** DOM mouse-up, and a click event for click trials. It polls read-only state, not input. Prior-trial terminal events cannot satisfy the next interval. A timeout means no terminal observation within the budget, not permanent loss.

## Current-build result

The [aggregate matrix](results/2026-10-04-webview-context.json) records macOS 27.0.1 build 26A434, arm64, WebKit bundle 22625.1.29.11.28, three displays, on 2026-10-04 (Asia/Taipei):

| Receiver / context | First single click | Subsequent single click | Canvas drag |
| --- | --- | --- | --- |
| Ordinary / inactive | No DOM/effect within three-second observation budget | Same | App receives 27 events; no DOM/effect |
| Ordinary / visible background | No DOM/effect within budget | Same | App receives 27 events; no DOM/effect |
| Pass-through observation / inactive | Native first-mouse result false; no view callback/DOM/effect | Same | Native policy false; no view callback/DOM/effect |
| Patched diagnostic control / inactive | Exactly one click effect | Exactly one additional click effect | One native down, 25 drags, one up; 28 DOM events including release, but `buttons` zero and object unchanged |

In the bounded control trial, terminal observations arrived after approximately 868 ms, 906 ms and 526 ms of polling for the first click, second click and drag respectively. Native up, DOM up and `dragging=false` are observed; displacement remains zero. The synthetic page explicitly requires a held left-button mask before moving an object, so the observed zero mask explains that page's missing displacement. It does not establish the installed WebKit's internal cause or a general solution.

Foreground, receiver activity and visible Spaces stayed unchanged at sampled resolution across the matrix. One pass-through drag recorded pointer movement and failed the stationary isolation check; this movement remains unattributed. Other latest cases passed that sampled check. These results do not guarantee all-app, physical-overlap or sub-sample isolation.

A separate read-only generic AX contrast saw seven window/chrome nodes and no uniquely observed page button in this inactive instance. It issued no AX press. This does not establish that all WebViews lack accessible page controls. Engine/platform research and exact source versions remain local; upstream behavior is not assumed to match this installed binary.

A later [bounded AX contrast](webview-ax.md) observed the ordinary page button on subsequent reads and demonstrated exactly-once effects using the existing AXPress route. No receiver or accessibility-mode changes were required. Those runs did not pass stationary isolation because pointer movement remained unattributed; they do not fix the raw-pointer or canvas results above.

The evidence supports two distinct receiver-side hurdles: native first-mouse refusal and, after a diagnostic policy change, insufficient DOM held-button state. It does **not** justify changing the real receiver, spoofing global button state, activating the target, switching Spaces, double-clicking or replaying uncertain input. A production route must work on an unmodified receiver and preserve binding, interruption and physical isolation before adoption. Accepted ordinary interrupted drag, physical WebView overlap and real-app coverage remain unverified.

## Verification

The fixture compiled with warnings as errors and the actual ordinary/observation/control matrix executed through the unchanged CLI. Offline tests require exact click counts, independent first/subsequent outcomes, displacement plus held-button and release evidence, bounded delayed-terminal observation, and rejection of prior events. They are not compatibility or fresh-agent acceptance.

## Repair work in progress

See [background-pointer-repair.md](background-pointer-repair.md) for sender candidates that failed the ordinary baseline, the timestamp hypothesis rejected by receiver evidence, and the conditions required before a functional correction is ready.
