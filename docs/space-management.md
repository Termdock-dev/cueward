# Existing Space discovery and background window moves

```sh
cueward space list
cueward space window --id 12345
cueward space move-window --target '<move_target>' --space 7
```

Space discovery reports displays, their current Space, and available Space IDs. `type: 0` identifies a user desktop; other types are not move destinations. `is_visible` is evaluated per display. IDs are current system identifiers, not durable desktop numbers; refresh the catalog before choosing a destination.

`space window` reads membership for a titled application window from `window list --all-spaces`. A window can belong to multiple Spaces. When membership is available, the result includes a `move_target` bound to the observed window and source Spaces, valid for five minutes. Empty membership omits the target. This observation requires no image capture. Listing membership does not prove that AX inspection or image capture is available.

## Move requirements

Use a fresh `move_target` from `space window`. The helper rechecks the window ID, PID, title, integer frame, token age, caller, and source Space membership. Changed source membership requires a new observation. The target app must be in the background. The destination must be an existing user Space that is not visible on any display.

The existing `input_target` from `window snapshot` is also accepted with its original identity and expiry checks. Snapshot targets do not bind source membership. A `move_target` cannot be used for keyboard, pointer, AX, or wait commands; it carries no image coordinates. Targets are observation references, not authorization credentials, and cannot distinguish replacements that reuse every bound attribute.

If any display's current Space is missing or malformed, discovery and moves stop. Unknown visibility is not treated as an inactive desktop.

Moves share the per-PID input lock with keyboard, scrolling, clicks, and drags. A busy app returns an error. The helper holds the lock through the membership check, including if its CLI caller exits after the move request. Once submitted, the asynchronous move cannot be cancelled or automatically undone.

The implementation uses optional private macOS Space queries and a dynamically discovered window-management operation. `move_window_available` means the entry point exists; it does not establish that every window accepts a move. An unavailable route is rejected without a fallback that switches desktops or activates apps.

The macOS command line tools, including `clang`, are required to build the temporary helper. Window metadata must remain available; image capture is needed only for the snapshot-target route. No additional package, background service, or VM is installed.

## Result and verification

`confirmed` means the window still belongs to the expected PID and its membership was read back as the requested Space. `sent_unverified` means that condition was not observed within two seconds. Neither status proves that the window's content or focus is unchanged.

Confirmation requires membership in exactly the destination Space. Membership that includes other Spaces remains `sent_unverified`, even when the destination appears in the list.

The result includes before/after memberships, `window_changed`, foreground PID endpoints, and visible Space endpoints. `foreground_changed` and `visible_spaces_changed` cannot detect a transient change that reverses between observations. Do not infer that a change was caused by Cueward solely from these fields.

Read `space window` again after a move to obtain current membership and a new move target. For coordinate input, take a new snapshot even if `window_changed` is false: geometry can change across displays, and previous image coordinates can become stale. AX exploration can use a fresh App observation when image capture is unavailable. On a timeout or uncertain result, read membership before deciding whether another move is appropriate. Do not replay automatically.

This command does not create, delete, rename, or switch Spaces. It only moves the specifically observed window. Standalone untitled windows remain outside the current catalog; inspect attached dialogs through their parent window when AX exposes them.
