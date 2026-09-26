# Observe a condition after an action

```sh
cueward window wait --target '<snapshot input_target>' \
  --condition value-equals --role AXTextField --identifier status --value ready
cueward window wait --target '<snapshot input_target>' \
  --condition enabled --role AXButton --name Continue --timeout-ms 10000
cueward window wait --target '<snapshot input_target>' --condition window-gone
```

Waiting sends no input and never replays the preceding action. Use the role, exact accessible name, or identifier from a current inspection. `--role` is required for element conditions; `--name` and `--identifier` narrow the match. `window-gone` omits element selectors. Only `value-equals` accepts `--value`, including an empty string.

Name selectors compare the same first 512 Swift characters displayed by `window inspect`. Longer names with identical displayed prefixes match the same selector; use an identifier to distinguish them. Duplicate matches remain ambiguous for value or enabled checks. `value-equals` compares the full AX value: an inspection's 512-character value preview is not evidence of the complete expected value.

## Conditions

| Condition | Evidence required |
| --- | --- |
| `element-exists` | At least one matching element was read. |
| `element-absent` | A complete traversal found no match. |
| `value-equals` | A complete traversal found exactly one match with the full expected AX value. |
| `enabled` | A complete traversal found exactly one match with AXEnabled equal to true. |
| `window-gone` | The observed window ID is no longer present in the available window catalog. |

Secure text values are not read. Duplicate matches for a value or enabled check return `ambiguous`. Traversal is bounded to 500 nodes and 12 levels; an incomplete tree cannot establish absence or uniqueness. AX read failures return an error rather than a successful absence result.

A node without a readable AX role also returns an error. This preserves the observation failure for the caller; it is not interpreted as absence or a normal timeout. Rediscover before continuing after an AX error.

## Results and recovery

Results report `matched`, `timed_out`, `window_gone`, `window_changed`, or `ambiguous`, along with poll count, elapsed time, observed match count, traversal completeness, and an optional current element ref. Text values are not echoed. `matched` establishes only the selected condition; it does not prove that a save, submission, or broader task succeeded.

`complete` describes AX traversal only. It remains false for `window-gone`, whose match is established by the window catalog without an AX traversal.

The snapshot target must be fresh at entry. Its window ID, PID, title, and integer frame remain bound throughout the wait. A renamed, resized, moved, or replaced window ends the wait with `window_changed`; take a fresh observation before continuing. A disappearing window returns `window_gone`, or `matched` when disappearance was requested.

If the window disappears or changes during AX binding or traversal, that in-flight poll can return an AX error before reaching the next catalog check. This is an uncertain observation: rediscover the window before continuing. No input is sent by the failed wait.

Timeout is 100–20000 ms, measured inside the helper after startup; poll interval is 50–1000 ms. An in-flight AX request can overrun the requested deadline. Helper startup and process timeout are separate bounds. The helper checks caller liveness between polls and stops when the caller exits.

After waiting, inspect again to obtain current action targets. If the condition times out or observation fails, inspect the actual state before deciding what to do. Do not automatically resend keys, clicks, saves, or submissions. Window actions and background input retain their existing result and release semantics.
