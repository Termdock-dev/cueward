# Retrieval

Use this reference when the user wants past knowledge, a digest, or something they saw before.

## Choose the right path

- Already indexed / searchable:
  `cueward search "<query>" --limit <N>`
- Fresh source read, not yet indexed:
  `cueward capture --source <safari|notes|messages|all> --since <duration>`
- Make fresh captures searchable:
  `cueward triage`
- Write back a digest:
  `cueward send --title ... --body ... --folder Cueward`
- Turn findings into a reminder:
  `cueward plan --title ... --notes ... --list Cueward`
- Reddit without Safari automation:
  `cueward reddit feed|post|search ...`

## Decision pattern

1. If the user is asking for something they saw before, try `search` first.
2. If search is empty or stale, use `capture` on the narrowest source.
3. Only run `triage` if the user needs the new data indexed for later search.
4. Interpret each command’s actual output before summarizing. On the unreleased reliability branch, capture uses a wrapped JSON array and search uses a wrapped JSON result array, including `[]`. Older installations print search results as JSON lines. Check installed help/version capabilities and preserve failure diagnostics.

## High-signal examples

```bash
cueward search "rust concurrency" --limit 5
cueward capture --source safari --since 7d
cueward capture --source all --since 24h
cueward capture --source all --incremental --since 7d
cueward triage
cueward send --title "Daily Digest" --body "Summary..."
cueward plan --title "Follow up" --notes "From digest"
cueward reddit search "async rust" --subreddit r/rust --limit 25
```

## Pitfalls

- `search` returns indexed fields, not full source-specific metadata. For files on disk, use `files search` with explicit root/depth when installed; see [files.md](files.md). It matches basenames/metadata, not file contents or Spotlight.
- If folder/sender/detail matters, use direct source reads instead of relying on `search`.

## Unreleased persistence and completion contract

These changes are not yet merged/published; check installed `capture --help` for `--incremental` before relying on them.

- Capture persists inbox/state even when only reading a source. Source failures and inbox/state save failures exit 1. Partial source successes remain available in the wrapped array and, if persistence succeeded, the inbox. Empty data with exit 1 is not an empty source.
- Inbox data is saved before advancing watermarks. Failed checkpoint writes can leave a saved batch; rerun is safe to index but inspect diagnostics first.
- Without `--incremental`, `--since` retains its time-window meaning. With it, each existing watermark takes precedence over `--since`; sources without a watermark bootstrap from that window. The boundary second is replayed to retain same-second records.
- Triage upserts repeated versions. Different native source IDs remain different records even if text and timestamps match. Tags do not define record identity. Legacy captures without IDs use their available contents/metadata, so indistinguishable old records cannot be separated retroactively.
- Search supports Han-character substrings and keeps the existing English/query syntax. It imports legacy stored rows once into `index-v2`, verifies their stored values, and retains the original index. Do not run old and new CLI versions against the same HOME concurrently; old-index writes after import are not mirrored.
- Pre-upgrade indexed rows remain separate legacy rows. Because their source IDs were never stored, a new capture can overlap a legacy row once; automatic cross-generation merging would risk collapsing distinct original records.

Repeated provider scans may still omit `data` with unchanged/skipped/warning/deleted status; preserve that distinction. These statuses are separate from the capture exit contract.
