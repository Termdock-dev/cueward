# Finder tag read, add and remove

Check installed command help. Select only the user's file/directory and requested exact names; tokens are observations, not authorization. Read returns file.version, tags_version and ordered name/stored-color entries. Reobserve after writes.

```bash
cueward files tags read --root /absolute/directory --path report.txt
cueward files tags add --root /absolute/directory --path report.txt --expected-version '<file.version>' --expected-tags-version '<tags_version>' --tag Work
cueward files tags remove --root /absolute/directory --path report.txt --expected-version '<fresh file.version>' --expected-tags-version '<fresh tags_version>' --tag Review
cueward files tags receipt --operation-id '<announced ID>'
```

Add/remove supported absent, empty or existing attributes while retaining unselected names/order/colors. Existing-name additions and absent-name removals are verified no-ops. New names use stored color 0; global Finder color/preferences are not edited. Names are case-sensitive without trimming, nonempty without NUL/CR/LF, at most 1024 UTF-8 bytes each and 256 names/8192 aggregate request bytes. No set-all, color-edit, link-following or download option exists.

Available files/directories/packages can be selected; links, aliases/unknown alias state, placeholders, special files and hardlinked-file edits are refused. Unknown plist representations/native-name disagreement and removal of all tags that would expose a legacy FinderInfo label are refused. Do not remove unrelated attributes to bypass these checks.

The original raw attribute is saved as original_attribute_base64 before writing. Inode-descriptor writes use CREATE for absence and REPLACE for presence, with fresh file/raw-attribute checks and an inode lock shared by Cueward editors. Finder/providers do not honor the lock. Existing-value replacement is not atomic CAS: a last-instant external edit can be overwritten. Do not promise isolation or automatically restore old evidence over newer tags.

Require status=completed and completion_verified=true, not merely outer Ok/lookup success. Not_started means known rejection, incomplete means write followed by failure, uncertain means tags may have changed. Preserve stderr ID and saved receipt; inspect actual state before any new decision. Worker timeout is 1..30000 ms/default 10000. Cancellation stops local work, not submitted metadata/provider work. No personal Finder preferences are changed.
