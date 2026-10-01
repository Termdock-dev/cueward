# Safari

Use this reference when the user wants current browser state or Safari-managed content.

## Live browser state

```bash
cueward safari tabs
cueward safari active
cueward safari read
cueward safari read --selector ".article-body"
cueward safari source
cueward safari exec "document.title"
```

- Use `tabs` / `active` / `read` for current Safari context.
- Use `--profile` when the user refers to a specific Safari profile.
- Use `--tab` when they mean one matched tab, not the frontmost tab.

## DOM interaction and diagnostics

```bash
cueward safari inspect --tab "Docs" --limit 200
cueward safari fill "#query" "hello" --tab "Docs"
cueward safari wait "#loading" --absent --tab "Docs"
cueward safari wait --text "Saved" --tab "Docs"
cueward safari exec --body "const x = await Promise.resolve(2); return x * 2;" --tab "Docs"
cueward safari batch --tab "Docs" --steps '[{"action":"click","ref":"<fresh ref from inspect>"},{"action":"assert","js":"document.querySelector(\"#menu\").getAttribute(\"aria-expanded\") === \"true\""}]'
cueward safari console --tab "example.com" --level error
cueward safari network --tab "example.com"
cueward safari network get 2 --tab "example.com"
```

Use inspect to choose current refs/selectors. A DOM ref expires after the next inspect, navigation or element removal; it is not a native AX target. Batch accepts at most 100 steps and reports a failed step's index. Follow page mutations with a relevant assert or wait. A timeout/error can leave earlier actions delivered; inspect before retrying instead of replaying the whole batch.

DOM click/key events are untrusted JavaScript events, not native keyboard editing or a trusted user gesture. Closed shadow roots and cross-origin iframe content are unavailable. Exec supports an awaited expression, or --body with an explicit return; wait --js and batch assert.js use synchronous expressions.

Console/network capture starts at the first diagnostic command in that page, ends on navigation/close, and keeps the latest 300 entries. It cannot recover earlier messages or browser-level traffic. Network covers fetch/XHR, with at most 16 KiB text preview for get. A missing earlier request is not proof no request occurred.

## Bookmarks

```bash
cueward safari bookmarks list --profile Work
cueward safari bookmarks search "claude" --profile Work --folder "Projects"
cueward safari bookmarks add --title "Claude" --url "https://claude.ai" --profile Work --folder "Projects/AI Tools"
cueward safari bookmarks delete --title "Claude" --url "https://claude.ai" --profile Work --folder "Projects/AI Tools"
```

- Folder paths use `/`.
- Deleting bookmarks requires both exact title and URL.

## Safari AI

```bash
cueward safari ai --provider gemini list
cueward safari ai --provider gemini read https://gemini.google.com/app/abc123
cueward safari ai --provider gemini prompt --prompt "台灣 AI 產業分析"
cueward safari ai --provider chatgpt save-images https://chatgpt.com/c/abc123 --output ~/Downloads
```

- Use this for browser-resident AI conversations and media workflows.
- Prefer `list` then `read` when the user asks about a prior AI conversation.

