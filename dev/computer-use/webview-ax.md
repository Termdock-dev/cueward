# Ordinary WebView buttons through existing AXPress

An ordinary, unmodified WKWebView exposed its synthetic page button on a later AX observation. The existing `window press` route then produced exactly one effect for each of two preplanned presses, on both visible-background and inactive Spaces. This is a useful semantic-button route, not a repair of raw pointer delivery or canvas dragging. Both action runs failed the stationary desktop check because pointer movement was observed and remains unattributed.

## Reproduce

```sh
python3 -B dev/computer-use/webview-ax-probe.py \
  --cli /absolute/path/to/cueward --parent /private/local/evidence \
  --context inactive
```

Choose `visible` for the visible-background contrast. The script requires an unlocked session and existing Accessibility, Screen Recording and event-post access. It owns a fresh ordinary receiver, uses the existing CLI for observation/wait/AXPress, and keeps raw data local. The receiver and observer expire after 120 and 90 seconds respectively; each CLI invocation is bounded to 45 seconds, the element wait to three seconds and post-action observation to three seconds. Cleanup stops only the owned receiver handle and removes owned snapshot files, including after errors.

No receiver subclass, first-mouse override, accessibility-mode setter, DOM injection, fake modifier, global input, cursor warp, app activation or visible-Space switch is used. Inactive setup moves only the owned window to an existing inactive user Space. The script uses the supplied synthetic task's button label and requires a single enabled AXPress target within the observed web area, matching the owned page's bounds. It never presses unnamed chrome buttons.

There are two separately named, preplanned actions. Each obtains a fresh AX target and dispatches once. Errors, uncertain outcomes, absent effects or reported foreground changes stop remaining actions. Observation polling does not resend input, and the second action is not a retry or a substitute for first-action success. `recorded` means evidence was collected, not that effect and isolation both passed.

## Current-build evidence

The [sanitized aggregate](results/2026-10-04-webview-ax.json) uses macOS 27.0.1 build 26A434, WebKit bundle 22625.1.29.11.28, arm64 and three displays. Four fresh receiver cases are retained:

| Case | AX exposure | Receiver effect | Desktop observation |
| --- | --- | --- | --- |
| Read-only visible contrast | First inspect: seven nodes, no page button; next three: twelve nodes, one named page button | No input dispatched | Foreground/visible Spaces stable, receiver inactive; pointer moved, stationary check failed |
| Read-only inactive contrast | Same seven-to-twelve-node transition | No input dispatched | Same limitation |
| AXPress visible background | Cold inspect: seven nodes; later bounded wait matched; both fresh action inspections: twelve nodes | First and subsequent press each increment counter by one, with one trusted DOM click and zero App mouse-event increment | Same limitation; maximum sample gap 43.0 ms |
| AXPress inactive | Same observation sequence | Same exactly-once effects | Same limitation; maximum sample gap 42.2 ms |

The read-only contrasts planned pauses of 0, 250, 1,000 and 2,000 ms before successive inspections. Helper and CLI execution time are additional; these pauses are not AX initialization latency measurements. In the action cases the wait matched on its first poll, after a cold inspect and snapshot. This does not prove that a wait was necessary, that the first inspection always starts initialization, or that the installed engine matches an upstream implementation. The observed transition is consistent with delayed AX exposure.

AXPress produced trusted DOM down/up/click events with `buttons=0` in these cases. The button counter proves the single-click effect; it does not supply the held-button state needed for a canvas drag. The native windows remained non-key/non-main and the owned apps stayed inactive at sampled resolution. Foreground and visible Spaces were stable and no owned activation event was observed. Pointer changes occurred in both read-only and action contrasts; that does not prove or disprove automation-caused movement. No complete isolation or physical-overlap pass is claimed.

## Agent workflow and remaining boundary

A first AX tree with no page controls is one current observation. `truncated=false` describes the traversal limits, not whether future asynchronous content will appear. Reinspect within a finite budget, or use an existing `element-exists` wait for a task/current-UI label. After a match, inspect again and use the fresh returned target. Do not use `matched_ref` as an action token, invent a label, press an unnamed window button or assume that absence is permanent.

```sh
cueward window snapshot --id <current-window-id>
cueward window wait --target '<fresh snapshot input_target>' \
  --condition element-exists --role AXButton --name '<known button label>' \
  --timeout-ms 3000
cueward window inspect --id <current-window-id> --depth 12 --limit 500
cueward window press --target '<fresh enabled page-button target>'
```

Verify the intended effect independently after one press. A timeout is a reason to report the missing current capability, not to activate the app, toggle unknown accessibility flags, double-click or automatically replay a possibly delivered request. Applications may activate themselves as an AX action side effect; endpoint results do not provide continuous isolation.

Raw pointer first-mouse refusal and canvas held-button state remain unresolved in [the original matrix](webview-context.md). These scripted diagnostics are not new fresh-agent task acceptance. #31 physical overlap, #32 generic accepted drag/interruption and real-app coverage remain open. Product routing, target tokens, guards and Rust/Swift helper code are unchanged; no new dependency is added.
