# Inspect a selected trash proposal

New source capability pending review. Check installed `files trash plan --help`. Use this read-only command to explain the exact entry and scope before a removal decision; actual trashing and recovery are unavailable. Never substitute permanent deletion or `files move` to an invented Trash path.

Observe the exact entry with `files info`, then pass its revision:

```bash
cueward files trash plan --root /absolute/directory --path Reports/old.txt \
  --expected-version '<observed entry version>'
```

Root is absolute; path is an explicit non-root relative entry without parent traversal. There is no default `.` target, glob/batch expansion, follow-links, destination, confirmation flag, execute, receipt or restore command. A literal filename containing `*` does not select other names. Planning does not move/remove anything, read payloads through Cueward's reading engine, enumerate descendants, stage a copy, allocate operation receipts, inspect Trash, activate Finder or request downloads.

Operation is `trash_plan`. Outer `Ok` / exit zero establishes observations, not deletion permission, completed trashing or recoverability. Inspect source/root/source-parent path/identity/revision, resources, target_kind and warnings. `requires_confirmation=true` describes a future removal decision; planning neither accepts nor remembers approval. `execution_supported` and `recovery_supported` are always false; `trash_destination=null` is unobserved, not an available or empty Trash directory. There is no receipt ID, status, content hash or execution token. Before any future removal, obtain explicit confirmation for the exact scope and fresh observations; never replay this JSON as a write.

`target_kind` distinguishes regular_file, directory, package_directory, finder_alias, symlink, special_entry and unknown. Unknown alias/package metadata remains unavailable/unsupported/error instead of false. A directory/package proposal concerns the whole entry; `whole_directory_entry` does not mean its contents were inspected. `descendants_inspected=false` and `link_targets_inspected=false` are always explicit. Alias files and leaf symlinks are selected themselves, not resolved targets; broken/outside-target links can be observed without querying those targets. Ancestor symlinks are refused. Explicit paths inside packages are not pruned by ancestor type.

Present warnings that affect scope or certainty: symlink_itself, alias_file_itself, unknown alias/package state, dataless/unknown source/root/parent availability, special entry and source/parent read-only mode. Read-only warnings describe Unix write bits, not TCC/ACL/volume or actual unlink permission. A placeholder observation never establishes provider coordination or availability for native trashing. Per-field errors are not empty tags/type values. A top-level error returns nonzero/outer `Err`, not a valid partial proposal.

Timeout defaults 10000 ms, range 1..30000 for the whole worker. Requests fit 16 KiB; a fixed 64 KiB pretty-JSON result budget applies before external escaping/envelopes. Overflow returns scan_limit without partial success. Root alias mapping, source and parent revisions are rechecked at return, but these sequential observations are not an atomic snapshot, reservation or source-inode lock. Directory revisions do not inventory descendant contents. Reobserve after changes and before any later operation.

`provider_coordination=filesystem_only_provider_state_unknown`; no implicit downloads or provider sync guarantee. Actual trash locations, interruption/recovery semantics, real provider/TCC/read-only/unmount/external-volume and older macOS acceptance are not verified by this plan. Keep user data in place and report the missing installed execution capability rather than inventing flags or a destructive fallback.
