# claude-tabs: a Claude Code aware tab bar for zellij

A drop-in replacement for zellij's stock `compact-bar` (same look and mouse behaviour, ported from
zellij 0.45.1) with extras for running Claude Code agents in tabs:

- **Claude marker** `✳` before every tab that runs Claude Code, animated with Claude Code's own
  spinner while the agent works, a number for running subagents (`✳2`), a badge plus a blinking tab
  when an agent finished in a tab you have not looked at yet.
- **Races**: every Claude tab gets a Warcraft III race (human, orc, undead, nightelf, naga, demon,
  neutral; unique up to 7 tabs), shown as a WC3 unit icon (colour icon font, kitty on Linux) or a
  unicode symbol, and the whole tab is tinted in the race colour.
- **Ding**: when an agent finishes (and no subagents are still running), a random Czech WC3 voice
  line of the tab's race plays (`sounds/wc3/`), falling back to `sounds/ding.wav`.
- Optional **auto-switch** to the finished tab, a **clock**, tab labels from the **folder** of the
  tab's main pane (your own tab names win), optional tab numbers.

## Layout

| Path | What |
|------|------|
| `src/`, `Cargo.toml`, `Cargo.lock` | plugin source (Rust, zellij-tile 0.45.1) |
| `claude-tabs.wasm` | prebuilt plugin (tracked, so no Rust toolchain is needed) |
| `setup.sh` | install / update (link, font, rebuild, permissions, hooks) |
| `dev.sh` | rebuild + hot reload in running sessions |
| `claude-hook.sh` | Claude Code hook: writes per-pane state for the plugin |
| `claude-agents.sh` | lists still-running background agents from Claude Code's transcripts |
| `claude-ctl.sh` | shell control: `ding`, `race`, `reshuffle` |
| `play-phrase.sh` | picks and plays a clip (also `--list`, `--ding`) |
| `os-focus.sh` | is the terminal window showing a session the OS-focused window? (`focused`/`unfocused`/`unknown`) |
| `claude-lib.sh` | shared POSIX helpers (state dir, player detection, timeout, ns clock) |
| `layouts/claude.kdl` | example layout (compact layout with the bar) |
| `sounds/ding.wav` | fallback chime |
| `sounds/wc3/` | voice lines, see `sounds/wc3/README.md` |
| `assets/wc3-icons/` | the WC3 icon font, its sources and build script |

All scripts find each other relative to their own location, so the checkout can live anywhere.

## Requirements

- zellij 0.45.x, a POSIX shell, `find`, `od`, `awk`, `sed`.
- An audio player: `pw-play` (PipeWire) or `paplay`, `ffplay`, `aplay` on Linux; `afplay` on macOS
  (built in). Autodetected.
- Claude Code with hooks (see below).
- `jq` for the automatic hook setup. Optional: kitty on Linux for the colour race icons; Rust with
  the `wasm32-wasip1` target only to rebuild the plugin.

## Install

```sh
git clone <this repo> ~/code/zellij-wc3cz-claude      # any location works
sh ~/code/zellij-wc3cz-claude/setup.sh                 # see below for hook targets / --no-hooks
```

`setup.sh` is idempotent. It

- links `~/.config/zellij/plugins/claude-tabs` to the checkout (`$ZELLIJ_CONFIG_DIR` or
  `$XDG_CONFIG_HOME/zellij` if set; an existing different path is left alone),
- on Linux installs `assets/wc3-icons/wc3-icons.ttf` to `~/.local/share/fonts/` (+ `fc-cache`),
- rebuilds `claude-tabs.wasm` when `cargo` and the `wasm32-wasip1` target exist (else keeps the
  prebuilt one),
- pre-grants the plugin permissions in zellij's `permissions.kdl` for the link path and the
  checkout path (no prompt),
- adds the Claude Code hooks (needs `jq`) with the absolute path of the checkout's `claude-hook.sh`
  to the Claude settings file(s): by default `$CLAUDE_CONFIG_DIR/settings.json` when
  `CLAUDE_CONFIG_DIR` is set, else `~/.claude/settings.json` (`CLAUDE_SETTINGS=FILE` still works).
  Several Claude configs (e.g. `~/.claude-personal`, `~/.claude-work`): `--settings FILE`
  (repeatable, replaces the default) and/or `--all-claude-dirs` (every `~/.claude*/` directory).
  Per file: backup `<file>.bak-claude-tabs` when it changes, untouched when the hooks are already
  there, hooks pointing at an older `claude-hook.sh` location are repointed; a target whose
  directory does not exist is skipped (nothing is created). `--no-hooks` skips this step.
  Restart running `claude` processes afterwards.

Then add the plugin to your zellij `config.kdl`:

```kdl
plugins {
    claude-tabs location="file:~/.config/zellij/plugins/claude-tabs/claude-tabs.wasm" {
        // package_dir "~/code/zellij-wc3cz-claude"   // only if the link above is not used
        ding_when_focused false
        auto_switch false
        race_dim "0.2"
        race_font "auto"
    }
}
default_layout "claude"
```

and copy `layouts/claude.kdl` into your zellij `layouts/` folder (or use
`plugin location="claude-tabs"` in your own layout instead of `compact-bar`). Start a NEW zellij
session (`zellij -s work`); running sessions keep their old bar and options.

kitty on Linux, for the colour race icons, in `kitty.conf` (then ctrl+shift+f5 or restart kitty):

```
symbol_map U+1F7E0-U+1F7E6 WC3 Icons
```

Side effect: the coloured-circle emoji U+1F7E0..U+1F7E6 render as WC3 icons in kitty.

macOS: same steps. The icon font is not installed: kitty on macOS cannot draw CBDT colour bitmap
fonts, so the plugin shows unicode symbols (`race_font "auto"`). Sounds play through `afplay`
(FLAC needs macOS 10.13+). Rebuilding needs
`brew install rustup && rustup-init -y && rustup target add wasm32-wasip1`; not needed otherwise.

To go back to the stock bar: `default_layout "compact"` and `compact-bar` in your layouts.

## Claude Code hooks

The plugin learns what Claude does from hooks in `~/.claude/settings.json` (`setup.sh` adds them;
restart running `claude` processes afterwards; every Claude config dir you use needs them). `<pkg>` =
the absolute path of the checkout:

| Hook event | Command |
|------------|---------|
| `SessionStart` | `<pkg>/claude-hook.sh start` |
| `UserPromptSubmit` | `<pkg>/claude-hook.sh working` |
| `PreToolUse` | `<pkg>/claude-hook.sh tool` (turns started by a finished background agent have no UserPromptSubmit) |
| `Stop` | `<pkg>/claude-hook.sh done` |
| `SubagentStart` | `<pkg>/claude-hook.sh substart` |
| `SubagentStop` | `<pkg>/claude-hook.sh substop` |
| `SessionEnd` | `<pkg>/claude-hook.sh end` |

Each entry has the form:

```json
{ "hooks": {
  "Stop": [ { "hooks": [ { "type": "command", "command": "/home/you/code/zellij-wc3cz-claude/claude-hook.sh done" } ] } ]
} }
```

The hook is a silent no-op outside zellij. It does not care which Claude config dir a session uses:
the transcript location comes from the hook payload (`transcript_path`). Without hooks a pane still counts as Claude when its
command contains `claude` or its title looks like Claude Code (`title_fallback`), but then there is
no working/finished state and no ding.

## Away from the terminal (OS focus)

zellij only knows which tab is active, not whether you are looking at the terminal at all. On every
`Stop` the hook asks `os-focus.sh` whether the terminal window that shows this zellij session is the
OS-focused window. If it is not (you are in the browser, on another workspace, ...), the finish is
flagged `away`: the active tab dings and blinks like a background tab, and stays unseen until the
plugin (polling `os-focus.sh` every 2 s) sees the window focused again. If focus cannot be determined
the result is `unknown` and the old behaviour applies (no ding for the active tab). Disable with
`ding_when_os_unfocused false`, or skip the check in the hook with `ZELLIJ_CLAUDE_OS_FOCUS=0`.

How it decides: it reads the focused window's pid and title from the desktop, then
1. title `<session> | ...` (zellij titles the host terminal like that) = focused;
2. else the focused pid is an ancestor of a zellij client attached to the session (Linux: found via
   `ss -xp` socket peers; elsewhere: any zellij client process) and the title does not name another
   running session = focused;
3. else unfocused. Every external command is capped at 1 s; typical cost 10-50 ms.

| Desktop | Backend | Status |
|---------|---------|--------|
| Hyprland | `hyprctl activewindow -j` | verified (kitty) |
| Sway | `swaymsg -t get_tree` (+ `jq`) | stub-tested only |
| X11 (i3, Xfce, KDE X11, GNOME X11, ...) | `xdotool`, else `xprop` | stub-tested only |
| KDE Wayland | `kdotool` (install it) | stub-tested only |
| macOS | `lsappinfo` (no permission needed; `osascript` fallback) | stub-tested only, pid match only (no title) |
| GNOME Wayland, others | none | `unknown` (old behaviour) |

Limits: windows of one multi-window terminal (kitty, wezterm, Terminal.app, iTerm2) share one pid; when
the title does not identify the session (always on macOS), another window of the same terminal counts
as focused. Sandboxed terminals (flatpak) do not match by pid; the title rule still works.

## Options

Set in the `claude-tabs` block of your zellij `config.kdl`. All values are strings or bare true/false; they are
read when the session starts (restart the session after a change; a plugin reload only picks up new
code).

| Option | Default | Meaning |
|--------|---------|---------|
| `package_dir` | `~/.config/zellij/plugins/claude-tabs` | the checkout; the wasm cannot know where it was loaded from, so the scripts and sounds are found here (`setup.sh` creates this link) |
| `ding_command` | `play-phrase.sh --ding` | fallback sound when no voice line can be played |
| `ding_when_focused` | `false` | also ding when the finished tab is the active one |
| `ding_when_os_unfocused` | `true` | ding and blink the ACTIVE tab too when the terminal window was not the OS-focused window at the finish (see "Away from the terminal") |
| `ding_with_subagents` | `false` | ding on every Stop, even while subagents run |
| `subagent_grace_s` | `8` | wait after the last subagent stops before dinging (main agent may continue) |
| `subagent_stale_s` | `900` | ignore subagent markers older than this (missed SubagentStop) |
| `working_hold_s` | `20` | any hook event keeps the working look this long |
| `auto_switch` | `false` | switch to a tab whose agent finished |
| `auto_name` | `true` | label tabs with the folder of their main pane (user-chosen names win) |
| `title_fallback` | `true` | detect Claude panes by command/title when there is no hook state |
| `tab_numbers` | `false` | prefix tab labels with their number |
| `clock` | `true` | `HH:MM` on the right |
| `show_other_clients` | `false` | stock `[ ]` markers of other attached clients |
| `active_underline` | `false` | underline the active tab |
| `subagent_count` | `true` | number of running subagents after `✳` |
| `spinner` | `true` | animated `✳` while working (`false` = static, italic) |
| `spin_ms` | `160` | spinner step (60-2000) |
| `blink` | `true` | blink finished-unseen tabs |
| `blink_ms` | `900` | blink phase (200-5000) |
| `blink_timeout_s` | `600` | stop blinking after this (0 = never) |
| `race_icons` | `true` | show the race icon |
| `race_font` | `auto` | colour WC3 icon font (auto = Linux on, macOS off; `true`/`false` force) |
| `race_tab_colors` | `true` | tint race tabs in the race colour |
| `race_dim` | `0.4` | brightness of inactive race tabs (0.1-1.0) |
| `race_all_tabs` | `false` | give races to tabs without Claude too |
| `races` | (empty) | fixed race pool, e.g. `"human orc undead"`; empty = folders of `sounds_dir` + all icon races |
| `sounds_dir` | (empty) = `<package_dir>/sounds/wc3` | voice line library `<dir>/<race>/*.flac` (empty = `sounds/wc3` next to `play-phrase.sh`) |
| `phrase_script` | `<package_dir>/play-phrase.sh` | clip picker (empty = only `ding_command`) |
| `agents_script` | `claude-agents.sh` next to `phrase_script` | running-agents lister |
| `player` | (empty) = autodetect | `afplay`, `pw-play`, `paplay`, `ffplay`, `aplay` |

## Commands

Run inside a zellij pane:

```sh
<pkg>/claude-ctl.sh ding [race]   # play a voice line now (default: active tab's race)
<pkg>/claude-ctl.sh race orc      # set the active tab's race
<pkg>/claude-ctl.sh reshuffle     # forget all race assignments, hand them out again
<pkg>/play-phrase.sh --list ""    # races that have playable clips
<pkg>/dev.sh [-s SESSION | --all] [--watch]   # rebuild + hot reload
```

## Sounds: pruning and adding

Active clips live in `sounds/wc3/<race>/`, excluded ones in `sounds/wc3/_excluded/<race>/` (never
played). Move files between them to prune or restore (`git mv`), effective with the next ding. A
new folder `sounds/wc3/<name>/` with audio becomes a new race (generic `◆` icon). Supported:
FLAC, WAV, MP3, AIFF; Ogg except with `afplay`; M4A only with `afplay`/`ffplay`. Details and
credits: `sounds/wc3/README.md`.

## Rebuilding the plugin

The config loads the TRACKED `claude-tabs.wasm`. After changing `src/`:

```sh
cd <pkg>
./dev.sh -s <session>      # cargo build --release --target wasm32-wasip1, copy to claude-tabs.wasm, hot reload
git add claude-tabs.wasm   # commit the rebuilt wasm together with the source change
```

Arch: `sudo pacman -S rust rust-wasm`. `target/` is git-ignored.

## Troubleshooting

- **Never hear a ding**: by default no ding for the ACTIVE tab while its window has the OS focus
  (`ding_when_focused`; `os-focus.sh SESSION` shows what the helper sees) and none while subagents run. `claude-ctl.sh ding human` must play a clip; if not, check the player
  (`pw-play <pkg>/sounds/ding.wav`). `claude` sessions started before the hooks were added
  send nothing: restart them. Hook state is in `/tmp/zellij-$(id -u)/zellij-claude/<session>/`
  (macOS: under `$TMPDIR`); `touch /tmp/zellij-$(id -u)/zellij-claude/debug` logs every hook event
  to `debug.log` there. Plugin errors: `grep claude-tabs /tmp/zellij-$(id -u)/zellij-log/zellij.log`.
- **Icons missing / boxes**: `fc-list | grep 'WC3 Icons'` must list the font (run `setup.sh`), then
  reload kitty (ctrl+shift+f5) or restart it. Other terminals: set `race_font "false"`. The font is
  tuned for kitty font_size 18 / cell_height 110% (`assets/wc3-icons/README.md` to retune).
- **New option has no effect**: options are fixed at session start; start a new session.
- **Old bar after an update**: sessions started before keep their plugin; `dev.sh -s SESSION` reloads
  the code, a new session picks up everything.
- **Permission prompt**: run `setup.sh` (pre-grants for `~/.config/zellij/plugins/claude-tabs/claude-tabs.wasm`
  and `<pkg>/claude-tabs.wasm`; zellij keys grants by the exact path in the alias).
- **No sounds / no agent counts, everything else fine**: the plugin cannot find the scripts; check that
  `~/.config/zellij/plugins/claude-tabs` points at the checkout or set `package_dir`.

## macOS checklist (not tested on a Mac)

Run in a zellij pane on the Mac and check the result:

```sh
zellij --version                                     # 0.45.x
sh <pkg>/setup.sh     # "macOS, WC3 icon font not installed", permissions
printf '%s\n' "$TMPDIR"; ls "${TMPDIR%/}"/zellij-$(id -u)/   # must contain zellij-log
sh <pkg>/claude-hook.sh start; ls "${TMPDIR%/}"/zellij-$(id -u)/zellij-claude/*/   # a file named like the pane id
afplay <pkg>/sounds/ding.wav
afplay "$(ls <pkg>/sounds/wc3/human/*.flac | head -n 1)"   # FLAC through CoreAudio
<pkg>/play-phrase.sh --list ""            # the 7 races
<pkg>/claude-ctl.sh ding human            # a human voice line
date +%z; uname -s                                   # clock offset / Darwin
```

Then check the hooks (`setup.sh` added them), restart `claude`, check that a finished agent in another
tab dings and blinks, `cd /tmp` in a pane and see the tab label follow after ~2 s, and look for
symbols drawn as colour emoji or double width (report which).

## Credits and licences

- Tab bar code ported from zellij's `compact-bar` (MIT, zellij contributors).
- Voice lines: Czech dub of Warcraft III, (c) Blizzard Entertainment, see `sounds/wc3/NOTICE`.
- Icons: Warcraft III button art, (c) Blizzard Entertainment, via https://wc3icons.coffbox.win/,
  see `assets/wc3-icons/README.md`.
- `sounds/ding.wav`: derived from a downloaded chime (see `AGENTS.md`).
