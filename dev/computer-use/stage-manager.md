# Input With Stage Manager On

Measured on 2026-11-07 on macOS 27.0.1 with Cueward 0.6.1, on the owner's working desktop: Stage Manager on, three displays, other agents and builds running. [Aggregate](results/2026-11-07-stage-manager.json).

## Why the fixtures did not find this

Every window fixture uses `.accessory` or `.prohibited` activation. Stage Manager does not manage those windows: an `.accessory` fixture kept its real frame in 3 of 3 runs. A `.regular` fixture stayed on the stage in some periods and became a thumbnail in others: 29 launches in a row were thumbnails, and the next 3 stayed on the stage. The cause is unknown. It did not follow the foreground app or the process name.

Real apps show the same split. On the same desktop, the off-stage windows of Discord, Dia and Termdock had thumbnail frames such as 107 by 98 against 1268 by 843 through Accessibility. On-stage windows matched. Snapshots of some of these thumbnails fail with `could not create image from window`.

After the fix, three read-only `input-status` sweeps over the real windows gave `ready` for every full-size background window. A Termdock thumbnail of 159 by 161 was refused. A foreground window was refused for being in the foreground. No full-size window was refused as a thumbnail.

## Outside the stage

A snapshot of such a window is a tilted picture from the Stage Manager strip, 196 by 194 pixels for a 720 by 472 point window. Its shape differs from the window, so pixels cannot be mapped to window points.

| Action | Before the fix | After the fix |
| --- | --- | --- |
| Click near the bottom of the picture | `sent_unverified`, `no_safe_point`; landed at point (58.8, 96) of the real window and pressed a button near its top in 3 of 3 runs | Refused before any event in 3 of 3 runs |
| `window scroll` | `sent_unverified`, no effect in 3 of 3 runs | Refused before any event in 3 of 3 runs |
| `window type-text` | Refused: not the verified keyboard window | Same |
| `input-status` pointer route | `dispatch_ready: true` | `dispatch_ready: false` with the reason |
| `app press` | Pressed the right button, stage and foreground unchanged, in 3 of 3 runs | Same in 2 of 2 runs; a third stopped with `desktop is locked` after the desktop auto-locked |

## Disturbances on the stage

An owned window was changed around one command. Each case ran once, except the sheet button case, which ran six times.

| Change | Result |
| --- | --- |
| App quits before a click | `window disappeared; take a new snapshot` |
| App quits during a drag | `partially_sent`, `window changed` |
| Window moves before a click or text | Refused: `window changed` |
| Window moves during a drag | `partially_sent`; the original receiver was released |
| Sheet opens before text | Refused: not the verified keyboard window |
| Sheet opens before a click | `sent_unverified`, no effect |
| Click aimed at a sheet button with the old snapshot | `sent_unverified`; the sheet stayed open and the button was not pressed in 6 of 6 runs |
| Sheet opens during a drag | Drag finished, `sent_unverified` |
| App hung, then a snapshot | Snapshot succeeded |
| App hung, then a click | Refused after 3 s; nothing was delivered after it resumed |

Clicks are routed to the window ID, so a sheet, which is a separate window, did not receive them. These runs used one `NSAlert` sheet, in `.accessory` and `.regular` fixtures. Other sheets and apps were not measured.
