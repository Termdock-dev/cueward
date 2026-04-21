# Shortcuts

Use this reference when the user wants to inspect, create, edit, or run Apple Shortcuts.

## Core commands

```bash
cueward shortcuts list
cueward shortcuts show --name "Clean URL Share"
cueward shortcuts create "Clean URL Share"
cueward shortcuts run --name "Clean URL Share"
cueward shortcuts rename --name "Clean URL Share" "Clean URL Share v2"
cueward shortcuts move --name "Clean URL Share v2" "Utilities"
```

## Surfaces and input types

```bash
cueward shortcuts input-type --name "Clean URL Share" url
cueward shortcuts surface --name "Clean URL Share" share-sheet
cueward shortcuts surface --name "Clean URL Share" library-root
```

## Incremental action editing

```bash
cueward shortcuts add-text --name "Clean URL Share" --value "hello"
cueward shortcuts add-get-urls --name "Clean URL Share" --from extension-input --output urls
cueward shortcuts add-get-text --name "Clean URL Share" --from urls --output url_text
cueward shortcuts add-replace-text --name "Clean URL Share" --from url_text --find "hello" --replace "world"
cueward shortcuts add-copy-to-clipboard --name "Clean URL Share" --from text_2
cueward shortcuts add-share --name "Clean URL Share" --from text_2
cueward shortcuts add-if --name "Clean URL Share" --input text --value world --then-actions then.yaml
cueward shortcuts add-repeat --name "Clean URL Share" --input urls --body-actions repeat.yaml
```

## Spec workflow

```bash
cueward shortcuts validate-spec clean-url-share.yaml
cueward shortcuts apply clean-url-share.yaml
cueward shortcuts export-spec --name "Clean URL Share"
```

## Rules

- Prefer `show` / `export-spec` / `apply` when the user is reasoning about shortcut structure.
- Selector-based commands accept either `--name` or `--id`.
- For larger edits, prefer spec files over long incremental command chains.

