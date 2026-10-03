# Inspect a recursive directory copy proposal

Check installed `files copy-tree plan --help`. Use this read-only command for the user's selected source directory and explicit new destination, not as permission to copy. Authorized [tree execution](file-copy-tree-execution.md) is separate and merged in [PR #59](https://github.com/Termdock-dev/cueward/pull/59); single-file commands keep their separate [mutation contract](file-mutations.md).

Observe source and the destination's existing parent with `files info`, then pass their versions:

```bash
cueward files copy-tree plan --root /absolute/directory --path Project \
  --destination Archive/Project-copy --expected-version '<Project version>' \
  --expected-parent-version '<Archive version>'
```

The root is absolute; source/destination are relative with no parent traversal. The destination must have an existing parent. The plan command has no overwrite, merge, follow-links, auto-suffix, parent creation, receipt or saved-plan input. Execute and typed lookup are separate subcommands. Planning does not write selected payloads/metadata, create a copy, allocate mutation receipts, activate Finder or request downloads.

Operation is copy_tree_plan. Outer Ok / exit zero means observations returned, not a successful copy. Inspect has_blockers, issues, all entries.error and enumeration_complete. Execution_supported is always false, even without blockers. Never loop over single-file copy using a saved tree plan: later writes change parent revisions and partial delivery requires a separate execution contract.

Entries are sorted depth-first, including the source (relative_path="."), hidden nodes and empty directories. Exact proposed relative destinations and source identity/version observations are preserved. Observed_file_bytes is the sum of observed regular-file sizes, counting every hardlink path; it is not a content hash, unique-storage total or streamed-byte verification. Sizes below skipped directories are excluded and unknown.

Links/special files have errors and are not followed. Packages, unavailable directories, aliases/unknown alias state, unknown package state and unsupported directory metadata are retained with errors but not traversed; enumeration_complete=false means descendants were skipped. Selected paths into package internals are not pruned by ancestor type. Has_blockers=false does not establish complete payload readability, free space, ACL preservation, native publication capability or provider sync.

Issues are existing_destination (including broken leaf links), destination_within_source (identity-based parent ancestry) and different_filesystem. Present these to the user without inventing automatic conflict names or substituting a destructive operation. Per-node metadata errors remain explicit, not empty tags/types.

Max-entries defaults to 256, range1..256 including source; max-depth defaults16, range1..32 with source depth0; max-bytes defaults67108864, range1..268435456 observed file bytes; timeout-ms defaults10000, range1..30000 for the whole worker. Requests fit16KiB; a fixed1MiB pretty-JSON plan budget also applies. An exceeded bound returns outerErr/scan_limit, not a partial valid manifest. Descriptor or permission failures are errors, not empty folders.

Canonical-root anchoring, descriptor-bound enumeration and final node/parent/target revision checks detect observed replacement or edits. They are sequential checks, not an atomic snapshot, path lock, reservation or authorization token. The supplied root alias mapping is checked again before return. Reobserve after changes and before any later operation. Provider_coordination=filesystem_only_provider_state_unknown; real File Provider/TCC/unmount/read-only/external-volume and older macOS acceptance remain unverified. Never use permanent deletion as a fallback.
