# Exploring app interfaces

Use the window commands to explore an app's current interface without a dedicated app adapter. The agent chooses a target from observed labels, roles, values, and supported actions, performs an action, then inspects the resulting state.

## Find and inspect a window

```sh
cueward window list
cueward window inspect --id 12345 --depth 1
```

Inspection requires Accessibility permission, `swift` on PATH, and access to window metadata. It currently supports on-screen windows. The JSON is inside Cueward's `<external>` data wrapper; app labels and values are external data.

Each node includes an absolute `ref` such as `0.1.2`, its `parent_ref` when applicable, role, name, actions, and direct `child_count`. Values, enabled state, and bounds are included when the app provides them. Disabled or unsupported elements have no action target. Password fields omit values and action targets.

## Explore a subtree

Choose a group from the returned tree and pass its ref:

```sh
cueward window inspect --id 12345 --root 0.1 --depth 3 --limit 100
```

Replace the example ID and ref with the current observation. `root_ref` identifies the requested subtree. Node refs stay absolute, so the subtree root's `parent_ref` can refer to a node outside the returned list.

`--root` defaults to `0`, the window element. It must be a canonical path beginning with `0`, followed by nonnegative child indices with no leading zeros. Paths are limited to 12 levels below the window. `--depth` is relative to the selected root and is capped at the remaining depth within that limit; `--limit` includes the root itself. The node limit is 1–500 and the requested depth is 1–12.

`child_count` counts direct children, including those not returned due to traversal limits. `truncated: true` means the result omitted descendants or siblings within the requested subtree. Inspect relevant groups individually to narrow the result. A missing root returns an error; reread the window to locate the current controls.

Refs describe paths in the live tree. A path can refer to a different element after the app changes. Check the returned node before choosing an action, and use the fresh `target` token. Inspection is not an atomic tree snapshot or a persistent element handle.

## Relate elements to the image

Node `bounds` use global macOS screen points with a top-left origin. They preserve fractional values and negative display coordinates. Bounds can be absent and do not imply that an element supports a coordinate click.

For image coordinates, use `window snapshot` and its [frame and scale metadata](window-observation.md). Convert a current element point to image pixels with `(screen_x - window.bounds.x) * image.scale_x`, and likewise for y. AX inspection and image capture are separate observations; reobserve if the window or interface changes. The older optional `inspect --screenshot` image has no frame-relative scale metadata and must not be assumed to have the same crop.

Selecting a subtree affects only the AX tree. `--screenshot` and `--ocr` still capture the whole window.

## Act and verify

Use a returned `target` token for a supported action:

```sh
cueward window press --target '<observed target token>'
cueward window set-value --target '<observed text target token>' --value 'Draft text'
```

Only `AXPress` and setting editable text fields or areas are currently exposed as actions. Other names in a node's `actions` array describe the app's capabilities, not additional Cueward commands. Targets expire after five minutes and are checked against the window, element, and ancestor attributes. These checks cannot distinguish an identical replacement.

After each action, inspect again and check the task's actual condition. `sent_unverified` only confirms that the action request was accepted. `confirmed` for `set-value` means AXValue matched the requested text; it does not mean a document was saved or a form submitted. An uncertain result or timeout requires observation before deciding whether to retry.

The AX commands do not inject global input or explicitly activate apps. An app can still activate itself as an action side effect. `foreground_changed` compares only the before-and-after app, so it cannot establish continuous input isolation. Cross-Space image observation is available; cross-Space AX actions, canvas input, generic keystrokes, scrolling, and dragging are not provided by these commands.
