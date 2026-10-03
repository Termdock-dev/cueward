# Copy or duplicate explicitly selected package contents

Source capability merged in PR #63. Check installed `files copy-tree plan --help` and `files copy-tree execute --help` for `--include-packages`. Use it only when the user's requested copy includes package contents. For a sibling duplicate, choose an exact new sibling destination; `files duplicate` remains regular-file-only.

```bash
cueward files metadata --root /absolute/directory --path Owned.app
cueward files copy-tree plan --root /absolute/directory --path Owned.app \
  --destination Owned-copy.app --expected-version '<source version>' \
  --expected-parent-version '<parent version>' --include-packages
# Obtain fresh observations before an authorized write.
cueward files copy-tree execute --root /absolute/directory --path Owned.app \
  --destination Owned-copy.app --expected-version '<fresh source version>' \
  --expected-parent-version '<fresh parent version>' --include-packages
cueward files copy-tree receipt --operation-id '<announced UUID>'
```

Read [tree planning](file-copy-tree.md) or [tree execution](file-copy-tree-execution.md) for the normal scope, guards, verification and receipt rules. Observe the existing destination parent (`.` for a root-level sibling). Keep the extension if the intended result should retain extension-based package presentation; arbitrary destination names do not guarantee Finder's package classification.

Without the option, package directories are skipped with blockers. With it, only native known package directories (`resources.is_package=available/true`) are traversed, including selected and nested packages. Unknown package state still blocks. Every child must pass ordinary tree validation: no links, aliases, special entries, placeholders/unknown availability or unsupported metadata. Do not bypass a blocker or copy only the supported subset. Many real App bundles contain symlinks and are still unsupported.

The option does not expand budgets: execution at most 64 entries, plan at most 256, depth at most 32, file bytes at most 256 MiB, timeout 1..30000 ms. Package internals count as separate nodes. It does not authorize copying, launching, installing, overwriting, merging, creating parents, cross-volume copy/delete or cleanup.

Operations remain copy_tree_plan/copy_tree/copy_tree_receipt; `include_packages=true` is retained in the request and saved receipt. Missing/false means legacy skipping, and false is omitted on serialization. Older strict-parsing CLI builds cannot read opt-in receipts containing this field; keep the receipt intact and use a compatible binary. Plan `execution_supported=false` stays unchanged. Require execution status=completed AND completion_verified=true. Saved lookup and outer Ok do not establish current completion. Source bytes are retained; files/dirs are verified in private staging before one guarded no-overwrite publication. On failure, preserve receipt/staging and inspect before a new decision, never automatically retry or delete.

The result verifies filesystem bytes, bounded xattrs, permissions/mtime and child sets, not runnable/signed/notarized application validity, document semantic correctness, ownership/ACL/birth/access time/flags preservation or provider sync. Synthetic known-package fixtures and actual CLI success do not establish all App/document formats, Finder UI or provider/TCC/older-macOS acceptance. Existing parent-lifetime/deadline behavior and sequential-check limits apply.
