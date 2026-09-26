# Existing Space discovery and background window moves

```sh
cueward space list
cueward space window --id 12345
cueward window snapshot --id 12345
cueward space move-window --target '<input_target>' --space 7
```

Space discovery reports displays, their current Space, and available Space IDs. `type: 0` identifies a user desktop; other types are not move destinations. `is_visible` is evaluated per display. IDs are current system identifiers, not durable desktop numbers; refresh the catalog before choosing a destination.

`space window` reads membership for a titled application window from `window list --all-spaces`. A window can belong to multiple Spaces. Listing membership does not prove that AX inspection or image capture is available.

## Move requirements

Use the `input_target` from a fresh window snapshot. The helper rechecks the window ID, PID, title, integer frame, token age, and caller. The target app must be in the background. The destination must be an existing user Space that is not visible on any display.

Moves share the per-PID input lock with keyboard, scrolling, clicks, and drags. A busy app returns an error. The helper holds the lock through the membership check, including if its CLI caller exits after the move request. Once submitted, the asynchronous move cannot be cancelled or automatically undone.

The implementation uses optional private macOS Space queries and a dynamically discovered window-management operation. `move_window_available` means the entry point exists; it does not establish that every window accepts a move. An unavailable route is rejected without a fallback that switches desktops or activates apps.

The macOS command line tools, including `clang`, are required to build the temporary helper. Existing window metadata and snapshot permission requirements still apply. No additional package, background service, or VM is installed.

## Result and verification

`confirmed` means the window still belongs to the expected PID and its membership was read back as the requested Space. `sent_unverified` means that condition was not observed within two seconds. Neither status proves that the window's content or focus is unchanged.

The result includes before/after memberships, `window_changed`, foreground PID endpoints, and visible Space endpoints. `foreground_changed` and `visible_spaces_changed` cannot detect a transient change that reverses between observations. Do not infer that a change was caused by Cueward solely from these fields.

Take a new snapshot after a move, even if `window_changed` is false. Window geometry can change across displays, and previous image coordinates can become stale. On a timeout or uncertain result, read membership before deciding whether another move is appropriate. Do not replay automatically.

This command does not create, delete, rename, or switch Spaces. It only moves the specifically observed window. Standalone untitled windows remain outside the current catalog; inspect attached dialogs through their parent window when AX exposes them.
