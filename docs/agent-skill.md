# Install and synchronize the Cueward skill

The maintained source is [skills/cueward-agent](../skills/cueward-agent/SKILL.md). Repo discovery links at `.agents/skills/cueward-agent` and `.claude/skills/cueward-agent` point to that same directory. Local Codex and Claude Code sessions in this checkout can discover the skill without a separately maintained copy.

Codex reads repo skills from `.agents/skills` and personal skills from `~/.agents/skills`, including symlinked folders. See [OpenAI's skill documentation](https://learn.chatgpt.com/docs/build-skills). Claude Code uses `.claude/skills` and `~/.claude/skills` for local sessions and supports symlinks too. See [Claude Code's skill documentation](https://code.claude.com/docs/en/skills). Locations were checked on 2026-10-01. A cloud session still needs an environment capable of executing this local macOS CLI.

## Use the skill in other projects

Run the following from the root of the checkout you intend to install. The example targets Codex; for Claude Code, set the target to `"$HOME/.claude/skills/cueward-agent"`. An existing installation is backed up outside discovery directories and compared before update. Additional custom files are retained; overlapping edits remain available in the backup.

```bash
cueward_skill_target="$HOME/.agents/skills/cueward-agent"
cueward_skill_backup="$HOME/.local/share/cueward/skill-backups/$(date +%Y%m%d-%H%M%S)/cueward-agent"
if [ -e "$cueward_skill_target" ] || [ -L "$cueward_skill_target" ]; then
  mkdir -p "$(dirname "$cueward_skill_backup")" &&
    cp -RL "$cueward_skill_target" "$cueward_skill_backup" &&
    diff -qr "$cueward_skill_target" "$cueward_skill_backup" || exit 1
fi
mkdir -p "$cueward_skill_target" &&
  cp -R skills/cueward-agent/. "$cueward_skill_target/" &&
  diff -q skills/cueward-agent/SKILL.md "$cueward_skill_target/SKILL.md"
```

Compare SKILL.md and references after updating; recover any needed local edits from the verified backup. A copied installation does not update with git pull, so repeat synchronization from the chosen version. Keep backups outside skill discovery paths to avoid loading an outdated same-name skill. Repair a dangling installation symlink before proceeding.

Invoke `$cueward-agent` explicitly in Codex or `/cueward-agent` in Claude Code, or let the host select it from its description. Reopen the agent session if an update does not appear. If both repo and personal copies exist, check which path was loaded rather than assuming their instructions merge.

## Update the CLI separately

```bash
command -v cueward
cueward --help
cueward app --help
cueward window --help
cueward space --help
```

A skill installs instructions, not the binary. PATH may still resolve an older crates.io installation. Check help before using a newly documented command; an unknown command is not a permission failure. To install the implementation from a chosen checkout:

```bash
cargo install --path crates/cli --locked --force
```

Main, published releases and open PRs have different capabilities. The initial `files list/info/read` slice was merged in [PR #44](https://github.com/Termdock-dev/cueward/pull/44). Check `cueward files search --help` before using the subsequent filesystem search reference. The skill uses a documented capability only when the installed CLI supports it; newer source instructions do not establish availability in main or a published release. See [computer-use progress](computer-use-progress.md) for merged work and outstanding desktop verification.

## Keep the source current

When changing agent-visible commands, flags, JSON, target consumers, resume positions or completion semantics, update the relevant skill reference and user documentation in the same PR. Change SKILL.md when routing/trigger scope changes; keep detailed examples in references. Check examples against current CLI help/parsing without dispatching real mutations or desktop actions. Distinguish pending implementation, unverified desktop behavior and published capability.
