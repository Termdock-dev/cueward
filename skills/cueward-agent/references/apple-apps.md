# Apple Apps

Use this reference for Apple Notes, Quick Notes, Reminders, Calendar, OCR, screenshots, clipboard, Voice Memos, and Stickies.

## Notes

```bash
cueward notes create --title "Title" --body "Body" --folder Cueward
cueward notes update --title "Title" --body "New content" --folder Cueward
cueward notes delete --title "Title" --folder Cueward
cueward notes move --title "Title" --from Cueward --to Archive
```

## Quick Notes

```bash
cueward quick-notes list
cueward quick-notes update --title "Note Title" --body "New content"
cueward quick-notes archive --title "Note Title" --to Archive
cueward quick-notes delete --title "Note Title"
```

- For removing items from the Quick Notes smart view, use `quick-notes archive`, not `notes move`.

## Reminders

```bash
cueward reminders list
cueward reminders list --list Work
cueward reminders today
cueward reminders create --title "Review PR" --due "2026-04-22 10:00"
cueward reminders update --id x-apple-reminder://123 --new-title "Review PR #114"
cueward reminders complete --title "Review PR"
cueward reminders delete --title "Review PR"
```

## Calendar

```bash
cueward calendar today
cueward calendar list --from "2026-04-11 09:00" --to "2026-04-11 18:00"
cueward calendar list --calendar Work
cueward calendar create --title "Team Sync" --start "2026-04-12 14:00" --end "2026-04-12 15:00" --calendar Work
cueward calendar update --title "Team Sync" --calendar Work --new-start "2026-04-12 14:30"
cueward calendar delete --title "Team Sync" --start "2026-04-12 14:00" --calendar Work
```

- Calendar and Reminders reads prefer EventKit when the terminal has read access.
- If reads are unexpectedly slow or empty, run `cueward doctor` and check permissions.

## OCR / Screenshots / Clipboard

```bash
cueward ocr ~/Desktop/screenshot.png
cueward screenshot --ocr
cueward screenshot windows
cueward screenshot window --id 12345 --ocr
cueward clipboard get
cueward clipboard set "Hello from cueward"
```

## Voice Memos / Stickies

```bash
cueward voice-memos list
cueward voice-memos read --id F45D4751-183C-4032-99F7-F1FE1F541BA2
cueward stickies list
cueward stickies create --title "Temp" --body "Remember this"
cueward stickies update --id sticky-1 --title "Updated title"
cueward stickies delete --id sticky-1
```

## Diagnostics

```bash
cueward doctor
cueward doctor --json
cueward doctor --live-safari
```

