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

