# Warcraft III in your terminal ⚔️

Every Claude Code agent you run in [zellij](https://zellij.dev) becomes a Warcraft III hero or unit, and when it
finishes its job it tells you, in Czech: Arthas, Thrall, Illidan, Jaina, a grumpy Peon and ~90 more, each with
their own voice lines from the Czech dub.

**Czech only:** all voice lines are in Czech, there is no English version.

- Each Claude tab gets its own character: portrait icon and a tab colour matching its race.
- When the agent is done, that character says a random line out loud, and tabs you haven't looked at yet blink.
- A spinning `✻` shows which tabs are busy, with a counter for subagents.

(The tab bar plugin that makes this possible is called `claude-tabs`.)

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
