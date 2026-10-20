# claude-tabs: a Claude Code aware tab bar for zellij

A drop-in replacement for zellij's stock `compact-bar` (same look and mouse behaviour, ported from
zellij 0.45.1) with extras for running Claude Code agents in tabs:

- **Claude marker** `✳` before every tab that runs Claude Code, animated with Claude Code's own
  spinner while the agent works, a number for running subagents (`✳2`), a badge plus a blinking tab
  when an agent finished in a tab you have not looked at yet.
- **Characters**: every Claude tab gets one Warcraft III character (92 with Czech voice lines, e.g.
  Arthas Menethill, Thrall, Illidan; unique up to 92 tabs), shown as the character's portrait icon
  (colour icon font, Linux with fontconfig) or a two-letter initials badge (any terminal). The whole tab is tinted in the
  character's colour: the race colour (human gold, orc green, undead purple, nightelf teal, naga
  blue, demon red, neutral grey) shifted a little per character, so neighbours differ but the race
  stays recognisable.
- **Ding**: when an agent finishes (and no subagents are still running), a random Czech WC3 voice
  line of THAT character plays (`sounds/wc3/`), falling back to `sounds/ding.wav`.
- Optional **auto-switch** to the finished tab, a **clock**, tab labels from the **folder** of the
  tab's main pane (your own tab names win), optional tab numbers.

## Layout

| Path | What |
|------|------|
| `src/`, `Cargo.toml`, `Cargo.lock` | plugin source (Rust, zellij-tile 0.45.1) |
| `claude-tabs.wasm` | prebuilt plugin (tracked, so no Rust toolchain is needed) |
| `.claude-plugin/`, `hooks/hooks.json` | Claude Code plugin + marketplace manifest: registers the hooks |
| `fontconfig/` | optional colour-icon font install for fontconfig terminals (`install.sh`, generated rules) |
| `setup.sh` | optional helper: status (default, changes nothing), `--font`, `--permissions`, `--migrate-to-plugin`, legacy options |
| `dev.sh` | rebuild + hot reload in running sessions |
| `claude-hook.sh` | Claude Code hook: writes per-pane state for the plugin |
| `claude-agents.sh` | lists still-running background agents from Claude Code's transcripts |
| `claude-ctl.sh` | shell control: `ding`, `character`, `race`, `reshuffle` |
| `play-phrase.sh` | picks and plays a clip (also `--list`, `--ding`) |
| `os-focus.sh` | is the terminal window showing a session the OS-focused window? (`focused`/`unfocused`/`unknown`) |
| `claude-lib.sh` | shared POSIX helpers (state dir, player detection, timeout, ns clock) |
| `layouts/claude.kdl` | example layout (compact layout with the bar) |
| `sounds/ding.wav` | fallback chime |
| `sounds/wc3/` | voice lines, see `sounds/wc3/README.md` |
| `assets/wc3-icons/` | the WC3 icon font, its sources and build script |

All scripts find each other relative to their own location, so the checkout can live anywhere.

## Requirements

- zellij 0.45.x, Claude Code with plugin support, a POSIX shell, `find`, `od`, `awk`, `sed`.
- An audio player: `pw-play` (PipeWire) or `paplay`, `ffplay`, `aplay` on Linux; `afplay` on macOS
  (built in). Autodetected.
- Optional: fontconfig on Linux for the colour icons (`fontconfig/install.sh`; the plugin detects the font
  with `fc-list`, without it the bar shows initials badges); `ss` (iproute2, Linux) and a desktop helper for
  the OS-focus check (see "Away from the terminal"; `jq` for Sway; macOS needs `perl` or `timeout` from
  coreutils); `jq` for `setup.sh --migrate-to-plugin` / legacy options and the hook status; Rust with the
  `wasm32-wasip1` target only to rebuild.
- Disk: the voice line library is about 135 MB. A git clone holds it once; `claude plugin install` keeps
  one more cached copy of the repo per version, so plan for roughly twice that.

## Quick start (no config edits)

Nothing in your existing config files has to change. Two steps:

1. **Claude Code plugin** (registers the hooks; in a shell, or the same as `/plugin ...` inside `claude`):

   ```sh
   claude plugin marketplace add crossatko/zellij-wc3cz-claude
   claude plugin install zellij-wc3cz@zellij-wc3cz
   ```

   Restart running `claude` sessions (or `/reload-plugins`). With several Claude config dirs, run it once
   per dir (`CLAUDE_CONFIG_DIR=~/.claude-work claude plugin ...`).

2. **zellij layout** with the bar (the one step that cannot be automated: zellij only loads plugins named
   in a layout or config). Start a session with the shipped layout, straight from the plugin's copy:

   ```sh
   zellij -n ~/.claude/plugins/marketplaces/zellij-wc3cz/layouts/claude.kdl
   ```

   or put this pane into your own layout instead of `compact-bar`:

   ```kdl
   pane size=1 borderless=true {
       plugin location="file:~/.claude/plugins/marketplaces/zellij-wc3cz/claude-tabs.wasm"
   }
   ```

   (With `CLAUDE_CONFIG_DIR` set, the path starts with that dir instead of `~/.claude`.) To make it your
   default, copy the layout to `~/.config/zellij/layouts/claude.kdl` and start with `zellij -n claude`, or
   set `default_layout "claude"` if you want to.

   **One-time permission prompt**: the first time, zellij asks in the bar whether the plugin may read
   and change the session state, run commands (the sound player, helper scripts) and receive pipes
   (`Allow? (y/n)`): focus the bar with a mouse click on it or `Alt+↓` (zellij's default move-focus key)
   and press `y`. The bar then shows the tabs and can no longer be focused; focus returns to your pane.
   zellij remembers the answer for exactly this wasm path (`permissions.kdl` in zellij's cache dir).
   Pressing `n` leaves the bar showing "permissions denied"; a new session asks again. To skip the
   prompt, pre-grant instead: `sh ~/.claude/plugins/marketplaces/zellij-wc3cz/setup.sh --permissions`.

The marketplace clone is only refreshed by `claude plugin marketplace update zellij-wc3cz`: until then the
wasm the layout loads can lag behind the hooks of a newer plugin version.

That is all: voice lines, agent counts and attention markers work once a `claude` in that session has
run a hook (the hook tells the plugin where the package is). Plugin options go into the `plugin` block
of the layout (see Options), e.g. `plugin location="file:..." { ding_when_focused false; }`.

**Colour icons (optional, Linux)**: without the WC3 icon font the bar shows initials badges. With it,
each character gets its portrait:

```sh
sh ~/.claude/plugins/marketplaces/zellij-wc3cz/fontconfig/install.sh            # --uninstall to undo
```

It writes two files that belong to this package (`~/.local/share/fonts/wc3-icons.ttf` and
`~/.config/fontconfig/conf.d/60-wc3-icons.conf`) and runs `fc-cache`; it edits no terminal config.
The rules make fontconfig use "WC3 Icons" for the font's character icon codepoints (U+1FA70..U+1FAD6 minus
gaps, one per character), ahead of other emoji fonts; every other emoji and all text are unchanged. The
seven race fallback codepoints U+1F7E0..U+1F7E6 (the orange to brown circle emoji) are NOT overridden, so
the usual status-dot emoji keep their look; a character without an icon in the font then shows the system
circle emoji where the race glyph would be. Side effect: in every app that uses fontconfig the few emoji
at the character codepoints (ballet shoes, ringed planet, chair, hamsa, ...) show WC3 portraits.
Verified only through `fc-match` and kitty's own fontconfig query (not in a running terminal window of
every kind). Per terminal:

| Terminal | Colour icons after `fontconfig/install.sh` |
|----------|--------------------------------------------|
| kitty | expected to work through its per-character fontconfig fallback (restart kitty); the kitty fontconfig query was checked with `fc-match`; a `symbol_map ... WC3 Icons` line is not needed but harmless |
| alacritty | needs an extra fontconfig family rule of your own (append `WC3 Icons` to your terminal font's family; `fontconfig/install.sh` prints an example) and probably `font.offset.y`, see `assets/wc3-icons/README.md`; not tested here |
| wezterm, foot, konsole | untested |
| ghostty | no: it ignores fontconfig for emoji; add `font-codepoint-map = U+1FA70-U+1FAD6=WC3 Icons` to its config yourself |
| macOS terminals | not supported (CoreText draws no CBDT bitmaps): initials badges |

A terminal that cannot draw colour bitmap fonts shows another emoji or a box instead: set
`race_font "false"` in the plugin block (initials badges) or run `fontconfig/install.sh --uninstall`.

macOS: the same two steps. Sounds play through `afplay` (FLAC needs macOS 10.13+).

## Uninstall

1. Bar: start zellij without the layout (or `default_layout "compact"`) and remove the pane from your own
   layouts.
2. Hooks: `claude plugin uninstall zellij-wc3cz@zellij-wc3cz`, then `claude plugin marketplace remove
   zellij-wc3cz` (run both once per Claude config dir you installed it in). Hooks that an older `setup.sh`
   wrote into `settings.json`: `sh <pkg>/setup.sh --migrate-to-plugin` (before deleting the checkout).
3. Icon font: `sh <pkg>/fontconfig/install.sh --uninstall` (or `setup.sh --font-uninstall`) removes
   `~/.local/share/fonts/wc3-icons.ttf` and `~/.config/fontconfig/conf.d/60-wc3-icons.conf`. While the font
   is installed it also answers for the emoji at the character codepoints in every fontconfig app.
4. zellij permission grant (only if you answered `y` or ran `setup.sh --permissions`): delete the block whose
   header is the path of `claude-tabs.wasm` from zellij's `permissions.kdl` (Linux
   `${XDG_CACHE_HOME:-~/.cache}/zellij/permissions.kdl`, macOS
   `~/Library/Caches/org.Zellij-Contributors.Zellij/permissions.kdl`).
5. Runtime state: `rm -r /tmp/zellij-$(id -u)/zellij-claude` (macOS: under `$TMPDIR`); it is recreated
   by the next hook event.
6. A git checkout can then be deleted; the plugin cache `~/.claude/plugins/` is cleaned by the commands in 2.

## Install from a git checkout (development, advanced)

```sh
git clone https://github.com/crossatko/zellij-wc3cz-claude ~/code/zellij-wc3cz-claude   # any location
claude plugin marketplace add ~/code/zellij-wc3cz-claude      # local marketplace: loaded in place,
claude plugin install zellij-wc3cz@zellij-wc3cz               # edits apply at the next claude start
zellij -n ~/code/zellij-wc3cz-claude/layouts/claude.kdl       # after changing its location= to the checkout
```

(Or try the hooks for one session only: `claude --plugin-dir ~/code/zellij-wc3cz-claude`.) The layout's
`location` must be the absolute (or `~`) path of the checkout's `claude-tabs.wasm`; a relative path is
resolved against the session's working directory, not the layout file.

`setup.sh` is an optional helper. Without options it prints the status (hooks per Claude config dir,
double registration, layout snippet, permission grant, font) and changes nothing. Each change needs its
own option:

- `--font` / `--font-uninstall`: `fontconfig/install.sh` (Linux).
- `--permissions`: pre-grant the plugin permissions in zellij's `permissions.kdl` for this checkout's
  wasm path (zellij keys grants by the exact `~`-expanded path, symlinks not resolved); the file is
  rewritten atomically and keeps its other content.
- `--build`: rebuild `claude-tabs.wasm` (else the tracked prebuilt one is used).
- `--migrate-to-plugin`: for users of an older setup.sh that wrote the hooks into `settings.json`:
  removes exactly those `claude-hook.sh <event>` entries (other hooks and keys stay; backup
  `<file>.bak-claude-tabs`; idempotent). Without it the plugin's hooks and the old entries both run.
  The script drops the duplicate event, but that doubles process spawns and relies on a marker file, so
  keep one route. Targets: `--settings FILE` (repeatable), `--all-claude-dirs` (every `~/.claude*/`
  dir), default `$CLAUDE_CONFIG_DIR/settings.json`, else `~/.claude/settings.json`.
- Legacy, not recommended: `--legacy-settings-hooks` (write the hooks into `settings.json` instead of
  using the plugin; old name `--hooks`), `--link` (the old `~/.config/zellij/plugins/claude-tabs` link
  for configs that still use the alias `file:~/.config/zellij/plugins/claude-tabs/claude-tabs.wasm`).

Sessions started before a change keep their old bar and options; start a new session.

## Claude Code hooks

The plugin learns what Claude does from hooks. The Claude Code plugin registers them
(`hooks/hooks.json`, commands `"${CLAUDE_PLUGIN_ROOT}/claude-hook.sh" <event>`, no matchers, default
timeouts); every Claude config dir you use needs the plugin installed. `<pkg>` = the package directory:

| Hook event | Command |
|------------|---------|
| `SessionStart` | `<pkg>/claude-hook.sh start` |
| `UserPromptSubmit` | `<pkg>/claude-hook.sh working` |
| `PreToolUse` | `<pkg>/claude-hook.sh tool` (turns started by a finished background agent have no UserPromptSubmit) |
| `Stop` | `<pkg>/claude-hook.sh done` |
| `SubagentStart` | `<pkg>/claude-hook.sh substart` |
| `SubagentStop` | `<pkg>/claude-hook.sh substop` |
| `SessionEnd` | `<pkg>/claude-hook.sh end` |
| `PermissionRequest` | `<pkg>/claude-hook.sh ask` (a tool approval dialog is about to show: attention) |
| `Notification` | `<pkg>/claude-hook.sh notify` (no matcher; only `permission_prompt`, `elicitation_dialog`, `elicitation_url_dialog` count, `idle_prompt` etc. are ignored by the script) |
| `PostToolUse`, `PostToolUseFailure` | `<pkg>/claude-hook.sh clear` (the question was answered) |

`PreToolUse` for `AskUserQuestion` / `ExitPlanMode` also raises attention. Attention = the character's
voice line plays (same rules as a finish: not for the focused tab unless `ding_when_focused` or the
window is not OS-focused) and the tab shows an inverted underlined `?` (blinking like a finished tab)
until the question is answered. One ding per request; options `attention` (default `true`) and
`attention_repeat_s` (default `0` = no repeat).

Legacy (`setup.sh --legacy-settings-hooks`) writes the same events into `settings.json` as
`{ "hooks": { "Stop": [ { "hooks": [ { "type": "command", "command": "<pkg>/claude-hook.sh done" } ] } ] } }`.
Use one route, not both: Claude Code runs a plugin's hook and an identical settings hook separately, so
both copies run for every event. `claude-hook.sh` drops the duplicate (a marker per event and payload), but
this doubles process spawns and depends on that marker, so migrate (`setup.sh` without options reports
a double registration; `--migrate-to-plugin` fixes it).

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

Set in the `plugin` block of your layout (`plugin location="file:..." { ding_when_focused false; }`), or in
a `claude-tabs` alias block of `config.kdl` if you load the plugin through such an alias. All values are
strings or bare true/false (`true/1/yes/on`, `false/0/no/off`); they are read when the session starts
(restart the session after a change; a plugin reload only picks up new code).

| Option | Default | Meaning |
|--------|---------|---------|
| `package_dir` | (empty) = found automatically | the package dir (scripts + `sounds/`). Empty: the dir `claude-hook.sh` runs from, which it reports to the plugin on every event (`<zellij tmp>/zellij-claude/pkgdir`; works from the Claude Code plugin cache, a checkout or a link); before the first hook event `~/.config/zellij/plugins/claude-tabs`. Set it only to force another location |
| `ding_command` | (empty) = the packaged `sounds/ding.wav` via `play-phrase.sh --ding` | fallback sound when there is no voice line to play (empty library, no player); a player that fails after starting does not trigger it |
| `attention` | `true` | a permission prompt, question or elicitation raises the `?` marker and plays the character's voice line (see "Claude Code hooks") |
| `attention_repeat_s` | `0` | repeat the attention ding every this many seconds while the request is unanswered (`0` = once) |
| `lead_in_ms` | `0` | play this much silence (0-3000) with the same player right before each clip, so a sleeping or Bluetooth speaker wakes on the silence instead of eating the start of the speech; adds the value plus about 0.15 s to every ding. Try `300` if clips start cut off |
| `dup_window_ms` | `2000` | a ding for the same character that follows another within this time is dropped (finish + question of one tab, two panes) |
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
| `race_icons` | `true` | show the character icon (colour font) or its initials badge |
| `race_font` | `auto` | colour WC3 icon font. `auto` = on when the hook found the `WC3 Icons` font installed (`fc-list`, re-checked every 10 min; macOS never), otherwise each tab shows a 2-cell initials badge (`AM` Arthas Menethill, `ČA` Čarodějka; unique for all 92 characters) on a chip of the character's colour, which works in any terminal. `true` forces the icon font, `false` forces the initials badges |
| `race_tab_colors` | `true` | tint tabs in the character's colour |
| `race_dim` | `0.4` | brightness of inactive tinted tabs (0.1-1.0) |
| `race_all_tabs` | `false` | give characters to tabs without Claude too |
| `characters` | (empty) | fixed character pool, comma separated (names have spaces), e.g. `"Thrall, Arthas Menethill, Illidan Stormrage"`; empty = every character that has clips |
| `races` | (empty) | only characters of these races, e.g. `"human orc undead"` (combines with `characters`) |
| `sounds_dir` | (empty) = `<package_dir>/sounds/wc3` | voice line library `<dir>/<race>/<Character> - <line>.flac` (empty = `sounds/wc3` next to `play-phrase.sh`) |
| `phrase_script` | `<package_dir>/play-phrase.sh` | clip picker (empty = only `ding_command`) |
| `agents_script` | `claude-agents.sh` next to `phrase_script` | running-agents lister |
| `player` | (empty) = autodetect | `afplay`, `pw-play`, `paplay`, `ffplay`, `aplay` |

## Commands

Run inside a zellij pane:

```sh
<pkg>/claude-ctl.sh ding [character]    # play a voice line now (default: active tab's character)
<pkg>/claude-ctl.sh character "Thrall"  # set the active tab's character (a name is required)
<pkg>/claude-ctl.sh race orc            # give the active tab a (least used) character of that race
<pkg>/claude-ctl.sh reshuffle           # forget all assignments, hand them out again
<pkg>/play-phrase.sh --list ""          # race<TAB>character for every character with playable clips
<pkg>/dev.sh [-s SESSION | --all] [--watch] [-p URL | -a [ALIAS]]   # rebuild + hot reload
```

## Sounds: pruning and adding

Active clips live in `sounds/wc3/<race>/`, excluded ones in `sounds/wc3/_excluded/<race>/` (never
played). Move files between them to prune or restore (`git mv`), effective with the next ding.
A character is the file name part before the first ` - `: put `New Name - line.flac` into a race
folder and it becomes a character (race icon, hashed colour shift) after the next library scan (30 s);
a new folder `sounds/wc3/<name>/` becomes a new race (generic `◆` icon). Supported:
FLAC, WAV, MP3, AIFF; Ogg except with `afplay`; M4A only with `afplay`/`ffplay`. Details and
credits: `sounds/wc3/README.md`.

## Rebuilding the plugin

The config loads the TRACKED `claude-tabs.wasm`. After changing `src/`:

```sh
cd <pkg>
./dev.sh -s <session>      # cargo build --release --target wasm32-wasip1, copy to claude-tabs.wasm, hot reload
git add claude-tabs.wasm   # commit the rebuilt wasm together with the source change
```

The reload is `zellij action start-or-reload-plugin file:<checkout>/claude-tabs.wasm`: it finds the running
plugin only if the layout's `location` is exactly that URL (otherwise pass `-p URL` with the layout's
location, or `-a` to reload through a `claude-tabs` alias from `config.kdl`). Arch: `sudo pacman -S rust
rust-wasm`. `target/` is git-ignored.

## Troubleshooting

- **Extra, doubled or cut-off speech**: one clip plays at a time (a ding for another tab waits for the
  running clip, at most 15 s, one waiting; same-character repeats within `dup_window_ms` are dropped).
  `/tmp/zellij-$(id -u)/zellij-claude/ding.log` has one line per attempt (trigger, tab, clip,
  played / failed / dropped-*). A clip whose start is cut off on a Bluetooth speaker: set `lead_in_ms 300`
  (or 500) and restart the session; judge by ear.
- **Never hear a ding**: by default no ding for the ACTIVE tab while its window has the OS focus
  (`ding_when_focused`; `os-focus.sh SESSION` shows what the helper sees) and none while subagents run. `claude-ctl.sh ding "Thrall"` must play a clip; if not, check the player
  (`pw-play <pkg>/sounds/ding.wav`). `claude` sessions started before the hooks were added
  send nothing: restart them. Hook state is in `/tmp/zellij-$(id -u)/zellij-claude/<session>/`
  (macOS: under `$TMPDIR`); `touch /tmp/zellij-$(id -u)/zellij-claude/debug` logs every hook event
  to `debug.log` there. Plugin errors: `grep claude-tabs /tmp/zellij-$(id -u)/zellij-log/zellij.log`.
- **Icons missing / boxes / wrong emoji**: `sh <pkg>/fontconfig/install.sh --check` shows what fontconfig
  picks (`WC3 Icons` expected for `charset=1fa70`); install with `fontconfig/install.sh`, then restart the
  terminal (kitty: ctrl+shift+f5). ghostty needs its `font-codepoint-map` line (Quick start table). An old
  kitty `symbol_map` must cover U+1FA70-U+1FAD6 (a stale U+F0100 range gives grey blobs) or be removed.
  Terminals that cannot draw it: `race_font "false"` (initials badges). The font is tuned for reference
  cells of 14x36 px (kitty font size 18, 110% line height; `assets/wc3-icons/README.md` to retune).
- **New option has no effect**: options are fixed at session start; start a new session.
- **Old bar after an update**: sessions started before keep their plugin; `dev.sh -s SESSION` reloads
  the code, a new session picks up everything.
- **Permission prompt every time / cannot answer it**: zellij keys the grant by the exact (`~`-expanded)
  wasm path in the layout, so a moved checkout or another path asks again. The prompt is answered in the
  bar: click it (or `Alt+↓`), then `y`. `<pkg>/setup.sh --permissions` pre-grants this package's path (run
  the `setup.sh` of the copy the layout points at).
- **Bar says "permissions denied"**: the prompt was answered with `n`. Start a new session and answer `y`,
  or pre-grant with `<pkg>/setup.sh --permissions`.
- **Hooks registered twice** (doubled process spawns, relies on the duplicate marker): the plugin and old
  `settings.json` hooks are both active; `<pkg>/setup.sh` shows it, `<pkg>/setup.sh --migrate-to-plugin`
  removes the old entries.
- **No sounds / no agent counts, everything else fine**: the plugin cannot find the scripts. It learns the
  package directory (`package_dir`, the folder with the scripts and `sounds/`) from the hooks (the file
  `pkgdir` in `/tmp/zellij-$(id -u)/zellij-claude/`), so start a `claude` in the session once (plugin
  installed?), or set the `package_dir` option in the layout's plugin block.
- **Bar and hooks disagree after `claude plugin update`**: the layout loads the marketplace clone's wasm,
  which only changes with `claude plugin marketplace update zellij-wc3cz`.

## macOS checklist (not tested on a Mac)

Run in a zellij pane on the Mac and check the result:

```sh
zellij --version                                     # 0.45.x
sh <pkg>/setup.sh     # status only; "WC3 icon font: not available on macOS"
printf '%s\n' "$TMPDIR"; ls "${TMPDIR%/}"/zellij-$(id -u)/   # must contain zellij-log
sh <pkg>/claude-hook.sh start; ls "${TMPDIR%/}"/zellij-$(id -u)/zellij-claude/*/   # a file named like the pane id
afplay <pkg>/sounds/ding.wav
afplay "$(ls <pkg>/sounds/wc3/human/*.flac | head -n 1)"   # FLAC through CoreAudio
<pkg>/play-phrase.sh --list "" | wc -l      # 92 characters
<pkg>/claude-ctl.sh ding "Thrall"         # a Thrall voice line
date +%z; uname -s                                   # clock offset / Darwin
```

Then check the hooks (`claude plugin list` shows zellij-wc3cz enabled), restart `claude`, check that a finished agent in another
tab dings and blinks, `cd /tmp` in a pane and see the tab label follow after ~2 s, and look for
symbols drawn as colour emoji or double width (report which).

## Credits and licences

- Tab bar code ported from zellij's `compact-bar` (MIT, zellij contributors).
- Voice lines: Czech dub of Warcraft III, (c) Blizzard Entertainment, see `sounds/wc3/NOTICE`.
- Icons: Warcraft III button art, (c) Blizzard Entertainment, via https://wc3icons.coffbox.win/,
  see `assets/wc3-icons/README.md`.
- `sounds/ding.wav`: derived from a downloaded chime (see `AGENTS.md`).
