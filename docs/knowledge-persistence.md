# Knowledge persistence and compatibility

Status: unreleased source changes for issues #95 through #101, not yet merged or published. Isolated CLI/native-boundary fixtures are distinct from live Notes write acceptance.

## Capture completion

`capture --source ... --since ...` preserves the complete time-window mode. `--incremental` instead uses each source's last successfully persisted watermark minus one second. `--since` bootstraps sources without a watermark, even when another source already has one. The overlap prevents same-second records from being skipped; it does not discover backdated records older than the checkpoint. Use a wider explicit time-window capture for those.

The inbox is durably published before watermarks advance. Source errors, inbox errors and checkpoint errors produce exit 1. Partial successful records remain in the output and in the inbox when saved. Failed checkpoint updates may leave a saved batch; re-indexing it is safe. Empty data with a failed exit does not prove the source is empty. Malformed state is reported rather than reset.

State updates are atomic and merge monotonic watermarks with the latest stored scan state. Capture, triage and index initialization have separate nonblocking advisory locks. The existing `fs4` dependency supplies cross-process locks that release on process exit; the lock files themselves may remain. A busy operation reports failure instead of racing another writer.

## Cue versions and retries

Triage commits upserts before archiving batches. A failed archive leaves pending data and is safe to retry. Archives never overwrite a different batch with the same name. Invalid inbox JSON fails visibly instead of being skipped.

Identity is a SHA-256 of canonical source, timestamp, content, title, URL and available metadata. Tags and attachment cache details are excluded. Safari visit IDs, Messages row IDs and Notes IDs are exposed as `metadata.source_id`; Notes also exposes `metadata.note_id` for mutation targeting. Different source IDs remain separate records with identical text/timestamps. An edited note version remains another historical cue when its timestamp/content changes. Source reads keep timestamp precision and propagate SQLite row errors.

Old captures without IDs use their available fields. Distinct old records that were already indistinguishable cannot be recovered from those captures; this is not a guarantee of retroactive source-level identity.

## Index v2

The new schema adds a stored exact cue key. Han characters have individual index positions; Tantivy's query parser treats consecutive query tokens as a phrase, supporting Chinese substrings without changing English word tokenization. This is character-based matching, not semantic word segmentation. Query operators and quoted English phrases remain available.

On first use, a new `~/.cueward/index-v2` is opened. Existing live rows from `~/.cueward/index` are copied with stable legacy IDs. Every stored row is compared after commit, including duplicate legacy rows, before a durable import receipt is published. An interrupted import replays upserts and rechecks the copy. The original index is not deleted, renamed or rewritten to the new schema. A malformed legacy index or import receipt fails rather than being treated as empty.

The first import takes extra disk space and time proportional to the existing stored rows. Existing inbox, processed batches and original index stay in place. Mixed old/new CLI writes against the same HOME are unsupported: writes to the original index after the one-time import are not mirrored. Legacy indexed rows lacked source IDs and are retained separately; a fresh source capture may overlap such a row once. They are not merged speculatively.

## External JSON

External responses use `print_external()`. Search now returns an array inside `cueward/search`, including an empty array. Calendar, Reminders, Quick Notes lists, Safari action state and browser-AI download results are wrapped as well. `<` and `>` inside the JSON are represented with JSON Unicode escapes, so JSON decoding restores original content rather than substituting HTML entities. Doctor output remains internal diagnostics. Shortcuts `export-spec` is a YAML exception: its flow-style body quotes string scalars before format-aware angle escaping and retains YAML enum tags. Extract the envelope body and load it as YAML, not JSON; the apply loader recovers the original string values.

## Verification boundary

The regression fixture runs the actual CLI with an owned temporary HOME and a fake `osascript` boundary. It never reaches user Notes or other applications. Pure index tests use RAM indexes; the legacy import test creates only owned temporary indexes. Live Notes targeting and write behavior still need a separately authorized disposable-note acceptance run. Do not install this source branch's skill over a released CLI without checking command help and output compatibility.

Full-suite validation uses `cargo test --workspace --locked -- --test-threads=1`. Existing Notes DB and doctor fixtures mutate process-wide HOME, with separate local mutexes rather than isolation from every other test. Parallel execution had six receipt-path failures; the serial full suite passed. CI therefore runs serially. Fixing the existing fixture isolation remains a recorded limitation, not a product fix in this change.

The CI runner is `macos-26`, not macOS 15: the newer kernel exposes `RENAME_RESOLVE_BENEATH`, required by the guarded-rename capability probe. The standard full test suite is retained; kernel-dependent tests are not skipped or weakened. See Apple's [macOS 26 XNU header](https://github.com/apple-oss-distributions/xnu/blob/xnu-12377.121.6/bsd/sys/stdio.h) and GitHub's [macOS 26 arm64 image](https://github.com/actions/runner-images/blob/main/images/macos/macos-26-arm64-Readme.md).
