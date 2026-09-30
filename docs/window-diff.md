# Compare a saved window snapshot

```sh
cueward window snapshot --id 12345 > previous.json
# After an operation, compare the same observed window with its current state.
cueward window diff --previous previous.json
```

The previous file can contain plain snapshot JSON or the exact `window/snapshot` external wrapper written by the CLI. Keep its PNG unchanged and readable. Default snapshot images have unique cache paths; reusing an explicit image output path can overwrite the historical evidence. Relative PNG paths resolve from the current working directory, as they do for snapshot output. A malformed snapshot or coordinate metadata returns a command error before observation.

`diff` lists windows across Spaces, finds the previous window ID, then uses the existing capture and identity checks. It sends no input and does not activate apps or switch Spaces. It does not compare Accessibility trees. Comparing observations does not establish that an action, save, or task succeeded.

| Status | Evidence |
| --- | --- |
| `unchanged` | The same observable target, frame, image dimensions and exact rendered pixels. |
| `content_changed` | The same coordinate mapping with one or more changed pixels. |
| `geometry_changed` | The frame, image dimensions, scale or origin changed. |
| `target_changed` | The ID, owning PID, app or title differs; images are not compared. A title change also requires rediscovery. |
| `window_gone` | A successful catalog read no longer contains the previous candidate. This includes a window that no longer meets catalog filtering, not only a closed window. |
| `observation_failed` | Catalog or fresh capture failed. Absence and unchanged content are unproven. |
| `not_comparable` | Fresh capture succeeded, but the historical or current PNG could not be decoded or compared. |

`previous_observation_id` and `current_observation_id` are SHA-256 identifiers of the respective serialized observations. They identify observation data, not credentials or durable window identity. `previous_window` and `observed_window` preserve the relevant target evidence. `current` contains the fresh full snapshot when the same target was captured successfully, including its own current `input_target`. An expired historical input token is not used for input or extended.

`geometry_changed` is null until a fresh capture of the same target is available. `content_changed` is null when pixels were not compared. A resize or change in image dimensions does not resample historical coordinates: it reports geometry change and leaves pixel comparison unknown. A translation with equal image dimensions can report geometry change and pixel evidence together. `coordinate_mapping_unchanged` is false on failures and geometry changes.

`pixels` reports the count of changed pixels, total pixels, and one bounding rectangle in current-image pixels from the top-left corner. Both PNGs are rendered into the same sRGB, premultiplied RGBA bitmap before exact comparison using macOS ImageIO. The decoder rejects mismatched dimensions, non-PNG input, dimensions above 16384 per axis, or more than 16777216 pixels. Missing, corrupt or oversized images remain uncomparable. PNG compression and file metadata alone do not constitute a pixel change.

`requires_reexploration` is false only for `unchanged`. An unchanged image still cannot prove unchanged AX references, focus, enabled state, hidden content, or app process identity beyond the bound observable fields. Pixel changes can come from animation or caret blinking. Coordinate mapping alone does not prove that the control at an old point is still the intended control. Reobserve controls before acting and retain the existing input guards. Identical replacements and transient changes between endpoint checks remain undetectable.

The new image remains in the ordinary screenshot cache, just like a normal snapshot. Cueward does not create a baseline database, retain a helper, or delete historical user files. Full `window snapshot` and `window inspect` remain available.

Unit tests compare real synthetic PNGs, including a changed pixel on the top edge, and cover moves, resizes, scale changes, replacement, disappearance, failed observations and unreadable history. The opt-in desktop test captures a disposable background AppKit window and checks a fresh unchanged result without activation.
