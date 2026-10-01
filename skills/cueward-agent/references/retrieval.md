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
4. Interpret each command’s actual output before summarizing. Capture uses wrapped JSON; search prints indexed result text, not a JSON result array. Preserve warnings about failed sources.

## High-signal examples

```bash
cueward search "rust concurrency" --limit 5
cueward capture --source safari --since 7d
cueward capture --source all --since 24h
cueward triage
cueward send --title "Daily Digest" --body "Summary..."
cueward plan --title "Follow up" --notes "From digest"
cueward reddit search "async rust" --subreddit r/rust --limit 25
```

## Pitfalls

- `search` returns indexed fields, not full source-specific metadata.
- If folder/sender/detail matters, use direct source reads instead of relying on `search`.

Capture persists inbox/state even when only reading from a source. A successful process can still report source warnings; an empty capture with a failed source does not establish an empty history. Repeated source scans may omit data with unchanged/skipped/warning/deleted status; preserve that distinction.
