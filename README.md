# claude-tabs 🧙

A friendly tab bar for [zellij](https://zellij.dev) that knows when your Claude Code agents are busy.

- A spinning `✻` shows which tabs run Claude, with a counter for subagents.
- Every Claude tab gets its own Warcraft III character (portrait icon and tab colour).
- When an agent finishes, that character says a random Czech line out loud. Tabs you haven't looked at yet blink.

## Install (two steps)

You need zellij 0.45.x and Claude Code.

```sh
# 1. the Claude Code plugin (registers the hooks)
claude plugin marketplace add crossatko/zellij-wc3cz-claude
claude plugin install zellij-wc3cz@zellij-wc3cz

# 2. start zellij with the bar
zellij -n ~/.claude/plugins/marketplaces/zellij-wc3cz/layouts/claude.kdl
```

The first time, zellij asks for permission in the bar: click it (or press `Alt+↓`) and press `y`.
Restart any running `claude` sessions and you're done.

Heads up: the voice lines make the download big (about 270 MB).

## Want the portraits?

Without the icon font you get little initials badges instead. For the real portraits:

```sh
sh ~/.claude/plugins/marketplaces/zellij-wc3cz/fontconfig/install.sh
```

Then add one line to your terminal config (kitty, ghostty, foot, alacritty and macOS are covered) and
restart the terminal. The details are in [docs/REFERENCE.md](docs/REFERENCE.md).

## More

Options, commands, troubleshooting, uninstalling and everything else live in
[docs/REFERENCE.md](docs/REFERENCE.md). The voice lines are described in `sounds/wc3/README.md`.

Warcraft III art and voices belong to Blizzard Entertainment; they are included here as fan content.
