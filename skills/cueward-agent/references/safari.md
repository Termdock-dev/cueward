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
cueward safari wait --url "/complete" --tab "Docs"
cueward safari wait --navigation --tab "Docs"
cueward safari exec --body "const x = await Promise.resolve(2); return x * 2;" --tab "Docs"
cueward safari batch --tab "Docs" --steps '[{"action":"click","ref":"<fresh ref from inspect>"},{"action":"assert","js":"document.querySelector(\"#menu\").getAttribute(\"aria-expanded\") === \"true\""}]'
cueward safari console --tab "example.com" --level error
cueward safari network --tab "example.com"
cueward safari network get 2 --tab "example.com"
```

A wait follows its tab while the URL and title change. Keep `--js` conditions read-only, because a closed tab to the left moves the index the wait polls.

Use inspect to choose current refs/selectors. A DOM ref expires after the next inspect, navigation or element removal; it is not a native AX target. Batch accepts at most 100 steps and reports a failed step's index. Follow page mutations with a relevant assert or wait. A timeout/error can leave earlier actions delivered; inspect before retrying instead of replaying the whole batch.

DOM click/key events are untrusted JavaScript events, not native keyboard editing or a trusted user gesture. Closed shadow roots and cross-origin iframe content are unavailable. Exec supports an awaited expression, or --body with an explicit return; wait --js and batch assert.js use synchronous expressions.

Console/network capture starts at the first diagnostic command in that page, ends on navigation/close, and keeps the latest 300 entries. It cannot recover earlier messages or browser-level traffic. Network covers fetch/XHR, with at most 16 KiB text preview for get. A missing earlier request is not proof no request occurred.

## JavaScript dialogs

`alert`, `confirm` and `prompt` stop the page. Safari JavaScript then times out after 15 seconds, and the error says a browser dialog may be open. Handle the dialog through Accessibility. Safari stays in the background.

```bash
cueward app list
cueward app inspect --pid <safari-pid>
cueward app inspect --pid <safari-pid> --root <observed-window-root> --depth 12 --limit 500
cueward app set-value --target '<fresh prompt field target>' --value '<text>'
cueward app press --target '<fresh dialog button target>'
```

Each Safari window is its own root, so check every window root the first command lists. The dialog is a node with subrole `AXDialog`, and its name is the message. Button names follow the system language. A `prompt` also has an `AXTextField`. The page receives the answer, so read the message and choose the button the user's task calls for. A dialog in a background tab of a window was not tested.

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
- Prefer `list` then `read` when the user asks about a prior AI conversation. Support differs by provider: Gemini has every action; ChatGPT has `prompt`, `list` and `save-images`; Grok has `prompt`, `list` and `read`.
- `--provider x` and `--provider threads` are social feeds, not AI conversations. They return posts, so never treat their output as a model reply: X `prompt` searches posts, X `list` reads the feed, X `read` reads one post, and Threads `list` reads the feed.
- Also available, with flags in `--help`: `safari open`, `close`, `close-tabs`, `click`, `scroll`, `scroll-and-read`, and for Gemini `ai mode`, `poll` and `save-media`.


## Unreleased output envelope

Not yet merged/published: all Safari and browser-AI JSON responses, including tab/action state and download paths, use an `<external>` envelope. Decode the JSON; embedded delimiters use JSON Unicode escapes and retain their original decoded text. Older installed commands may still print raw JSON for these responses.
