# Exploring application interfaces

Use App inspection when an application has no document window, or when its AX tree cannot be paired with a window in the system capture catalog. This includes applications exposing system Open/Save panels through another process. The commands explore the application's current Accessibility tree and require no application-specific adapter.

## Discover and explore

```sh
cueward app list
cueward app inspect --pid 123
cueward app inspect --pid 123 --root menu --depth 3
cueward app inspect --pid 123 --root w0 --depth 3 --limit 100
cueward app inspect --pid 123 --root w0.1 --depth 2
```

The first inspection lists window roots (`w0`, `w1`, ...) and the menu root (`menu`). Root discovery does not issue action targets. Choose a returned `ref` and inspect its subtree to obtain controls and eligible targets. Root refs are paths in the current observation; rediscover them after windows open, close, or change context.

Inspection accepts 1–500 nodes and 1–12 levels per request, with paths capped at 12 levels below an application root. `truncated` reports omitted descendants or roots. Explore relevant subtrees when limits are reached. Apps exposing more than 64 window roots are rejected.

Nodes include role, name, identifier when exposed, value, enabled state, actions, bounds, child count, and the actual AX `receiver_pid`. Names and values are previews capped at 512 Swift characters. Password values and targets are omitted. Disabled controls and non-leaf menu items do not receive targets.

`unavailable_attributes` lists attributes for which AX returned a generic failure. If only `AXDescription` is unavailable, independently verified actions and text assignment remain usable. Other partial reads suppress that node's targets while leaving descendants discoverable. Availability markers remain part of the target fingerprint, so changes require a fresh inspection. An absent value on a partial node does not establish an empty value. Missing or malformed roles, failed subrole reads, and malformed subroles stop inspection before reading field values; unsupported or absent subroles are allowed. Failed child enumeration, communication errors, and unreadable application roots also stop inspection. `truncated: false` describes traversal coverage, not attribute availability or an application's internal state.

## Act and verify

```sh
cueward app press --target '<fresh app node target>'
cueward app set-value --target '<fresh app text target>' --value 'Draft text'
```

`press` invokes AXPress. `set-value` replaces the value of an editable text field or text area, with a 65,536-byte input limit. Empty text is allowed. After opening a panel, discover roots again and inspect the exposed window or sheet before choosing an action.

Targets expire after five minutes. They bind the host PID, kernel process start time, executable path, current window/main/focused context, selected root, ancestor attributes, and node attributes. Each node fingerprint also includes its actual receiver process instance. Actions reobserve those bindings before dispatch. A changed or ambiguous observation requires inspection again. These are observable-identity checks: identical replacement nodes may remain indistinguishable, and tokens are observation references rather than authorization credentials.

Both the host app and actual receiver must be in the background at the dispatch check. The helper holds their per-process input locks, shared with targeted keyboard/pointer input. A busy lock stops the action. Inspection does not reserve a target or a lock for a later action.

`sent_unverified` means AXPress returned success; verify the intended effect with a fresh inspection or the task's resulting artifact. `confirmed` for text assignment means AXValue matched on readback, not that a document was saved or submitted. Errors and timeouts can follow delivery. Observe before choosing another action; Cueward does not replay automatically.

## Requirements and limits

An unlocked macOS session, Accessibility permission for the calling terminal, and the Swift command line tools are required. App AX inspection does not require a matching capture window. An unresponsive helper is bounded by its timeout.

The commands do not request activation, switch Spaces, or inject global mouse/keyboard events. An app may still activate itself as an AX side effect. `foreground_changed` compares only the before/after foreground PID, so it cannot rule out brief switches or a race with user input. Stop background work if the user brings the target or receiver forward.

App targets cannot be used with `window` actions, snapshots, raw input, or waits. Those commands retain their window identity requirements. For image or keyboard/pointer interaction, separately discover and bind a capture window using the [window workflow](window-exploration.md). App inspection does not infer that a service-owned surface is a capture window belonging to the host app. Apps that do not expose the required AX controls may still be unsupported.
