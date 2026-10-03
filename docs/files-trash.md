# Read-only trash proposals

`cueward files trash plan` implements the planning slice of [issue #42](https://github.com/Termdock-dev/cueward/issues/42). Read-only planning was merged in PR #60; check installed `--help`. It describes one selected filesystem entry before the user decides whether to trash it. Separately confirmed ordinary-file removal uses [verified trash execution](files-trash-execution.md); recovery is not implemented. Permanent deletion is excluded.

## Select and observe

Use one absolute root, an explicit non-root relative path and the version from `files info` on that exact entry:

```bash
cueward files info --root /Users/me/Documents --path Reports/old.txt
cueward files trash plan --root /Users/me/Documents --path Reports/old.txt \
  --expected-version '<observed entry version>'
```

The plan accepts no default selection, root-directory target, batch/glob expansion, link-following, destination or confirmation flag. Separate execute/receipt commands do not change its read-only contract; restore and permanent deletion are unavailable. A filename containing `*` is one literal filename, not a match pattern. This command never moves/removes the selected entry, opens payloads through Cueward's reading engine, enumerates descendants, stages a backup, creates a mutation receipt, looks up a Trash directory, activates Finder or requests a download. Native resource inspection reuses `files metadata`; read access times are not promised unchanged.

A successful plan is not authorization or proof that native trashing, deleting permissions or recovery are available. Present the exact selected path, entry kind and warnings to the user. Removal requires a separate explicitly confirmed execution request and fresh observations, not replay of this JSON.

## Result and scope

External operation is `trash_plan`. Outer `Ok` / exit zero means observations returned only. The result includes the original request and source/root/source-parent FileInfo identity/revisions, native `resources`, `target_kind` and `warnings`. It always has:

- `requires_confirmation=true`: a removal decision still needs explicit confirmation; this command does not accept or remember confirmation.
- `execution_supported=false` and `recovery_supported=false`.
- `descendants_inspected=false` and `link_targets_inspected=false`.
- `trash_destination=null`: no Trash location was queried, reserved or predicted.
- `provider_coordination=filesystem_only_provider_state_unknown`.

There is no operation ID, saved receipt, completion status or payload hash. Unknown metadata is retained as unavailable/unsupported/error, never invented as false/empty. A top-level inspection error instead returns structured `Err` and a nonzero exit, without a partial plan.

| `target_kind` | Selected entry |
| --- | --- |
| `regular_file` | Regular filesystem file with observed alias state false. Hardlink relationships are not resolved or changed. |
| `directory` | Observed non-package directory; the proposed scope is its whole entry, not a list of inspected descendants. |
| `package_directory` | Observed package directory; no bundle internals are enumerated. Explicit paths inside a package are not pruned by ancestor type. |
| `finder_alias` | Alias file itself, not its resolved destination. |
| `symlink` | Leaf link itself, including a broken link or one pointing outside root; its target is not queried. |
| `special_entry` | A special filesystem entry such as FIFO, observed without opening its payload. |
| `unknown` | Required source role metadata is unavailable, unsupported or errored, including a dataless file/directory. |

Directory/package entries carry `whole_directory_entry`; links carry `symlink_itself`; aliases carry `alias_file_itself`. Relevant metadata warnings include `alias_state_unknown`, `package_state_unknown`, `source_dataless`, `source_availability_unknown`, `root_availability_unknown`, `parent_availability_unknown` and `special_entry`. Source/parent `*_readonly_mode` warnings refer only to observed Unix write bits, not TCC, ACLs, volume writability or the ability to unlink the entry. A read-only file is not necessarily undeletable; no delete permission test is performed.

Only a leaf link can be inspected. Symlink ancestors, parent traversal and an absolute source path are rejected; the explicitly supplied root may be a stable alias to a directory. Root selection itself is not a valid trash target. Available resource fields use the existing bounded Foundation metadata query. Leaf links/special entries get not-applicable resource fields; dataless entries get unavailable fields without querying payload resources. These observations do not establish File Provider coordination, download completion or safety of a future mutation.

## Bounds and consistency

`--timeout-ms` defaults to 10,000 and accepts 1..30,000 for the whole read-only worker. Serialized requests fit 16 KiB. Results have a fixed 64 KiB pretty-JSON budget before external escaping/envelopes; exceeding it returns `scan_limit`, not partial apparently successful metadata. Native metadata allocation/latency is separate from the result bound.

Resolution is anchored to the initially canonical root. The original supplied spelling stays in the request; source and parent path/identity/revisions and root alias mapping/revision are checked again before returning. Detected changes discard the proposal. These are sequential metadata observations, not an atomic snapshot, namespace reservation, source-inode lock or directory-content inventory. A directory revision does not summarize all descendant payload revisions. Reobserve before a later decision, and do not use `move` or shell deletion to bypass the confirmed trash workflow.

## Verification and remaining work

Disposable native tests cover unchanged bytes/inode/link count/mode/mtime/ctime, exact Unicode/newline names, directories/packages, outside/broken leaf links, FIFO, read-only mode, stale/invalid/missing selection, stable/retargeted root aliases and ancestor-link refusal. Controlled platform seams cover aliases, unknown/dataless availability, per-field errors, unknown package state, source/parent changes during metadata inspection and result overflow. Actual CLI/worker tests cover JSON envelopes/escaping, failure exits, unavailable mutation options, strict request decoding and oversize/malformed input. A blocking-worker test covers the whole-worker deadline. No personal files or Trash items are moved or deleted.

Separate [ordinary-file execution](files-trash-execution.md) performs verified backup and guarded private quarantine before native Foundation trash, recording its actual resulting location. Planning itself still does not call that API or create receipts. Real provider/TCC/read-only/unmount/external-volume and older macOS acceptance remain unverified. #42 remains open for restore, broader trash types and other unfinished management work.
