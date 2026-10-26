# claude-tabs: a Claude Code aware tab bar for zellij

A drop-in replacement for zellij's stock `compact-bar` (same look and mouse behaviour, ported from
zellij 0.45.1) with extras for running Claude Code agents in tabs:

- **Claude marker** `✻` before every tab that runs Claude Code, animated with Claude Code's own
  spinner while the agent works, a number for running subagents (`✻2`), a badge plus a blinking tab
  when an agent finished in a tab you have not looked at yet.
- **Characters**: every Claude tab gets one Warcraft III character (91 with Czech voice lines, e.g.
  Arthas Menethill, Thrall, Illidan; unique up to 91 tabs), shown as the character's portrait icon
  (colour icon font, Linux with fontconfig) or a two-letter initials badge (any terminal). The whole tab is tinted in the
  character's colour: the race colour (human gold, orc green, undead purple, nightelf teal, naga
  blue, demon red, neutral grey) shifted a little per character, so neighbours differ but the race
  stays recognisable.
- **Ding**: when an agent finishes (and no subagents are still running), a random Czech WC3 voice
  line of THAT character plays (`sounds/wc3/`), falling back to `sounds/ding.wav`.
- Optional **auto-switch** to the finished tab, a **clock**, tab labels from the **folder** of the
  tab's main pane (`SSH: host` while an ssh session runs there; your own tab names win), optional tab numbers.

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
- Optional: the colour icon font (`fontconfig/install.sh`: fontconfig on Linux, `~/Library/Fonts` on macOS; the plugin detects the font
  with `fc-list` / the font file, without it the bar shows initials badges); `ss` (iproute2, Linux) and a desktop helper for
  the OS-focus check (see "Away from the terminal"; `jq` for Sway; macOS needs `perl` or `timeout` from
  coreutils); `jq` for `setup.sh --migrate-to-plugin` / legacy options and the hook status; Rust with the
  `wasm32-wasip1` target only to rebuild.
- Disk: the voice line library is about 135 MB. A git clone holds it once; `claude plugin install` keeps
  one more cached copy of the repo per version, so plan for roughly twice that.

## Quick start (no config edits)

An agent can do all of this for you: see `PROMPT.md` (the README has the prompt to paste). The manual steps follow.

None of the package's own scripts edits your zellij, terminal or Claude config files (the `claude` CLI
itself adds `extraKnownMarketplaces` and `enabledPlugins` to the config dir's `settings.json` when you run
the two commands below). Two steps. In the paths below, `${CLAUDE_CONFIG_DIR:-~/.claude}` is your Claude
config dir; zellij does not expand variables in a layout, so write the real path there:

1. **Claude Code plugin** (registers the hooks; in a shell, or the same as `/plugin ...` inside `claude`):

   ```sh
   claude plugin marketplace add crossatko/zellij-wc3cz-claude
   claude plugin install zellij-wc3cz@zellij-wc3cz
   ```

   Restart running `claude` sessions (or `/reload-plugins`). The marketplace add takes about 50 s and clones
   the repo (about 270 MB, half of it `.git`); the install adds a cached copy (about 140 MB). The clone
   uses your git config's transport (HTTPS, or SSH if you rewrite GitHub URLs to SSH, which needs a key).
   With several Claude config dirs, run both commands once per dir (`CLAUDE_CONFIG_DIR=~/.claude-work
   claude plugin ...`): with `CLAUDE_CONFIG_DIR` set the plugin lives under that dir, not `~/.claude`. One
   zellij layout (pointing at one copy of the wasm) serves all of them: each config's hooks come from its
   own copy and tell the plugin the package dir (the `pkgdir` state file).

2. **zellij layout** with the bar (the one step that cannot be automated: zellij only loads plugins named
   in a layout or config). Start a session with the shipped layout, straight from the plugin's copy:

   ```sh
   zellij -n ${CLAUDE_CONFIG_DIR:-~/.claude}/plugins/marketplaces/zellij-wc3cz/layouts/claude.kdl
   ```

   or put this pane into your own layout instead of `compact-bar`:

   ```kdl
   pane size=1 borderless=true {
       plugin location="file:~/.claude/plugins/marketplaces/zellij-wc3cz/claude-tabs.wasm"
   }
   ```

   (In the KDL pane write the real path, e.g. `$CLAUDE_CONFIG_DIR/plugins/...` expanded.) The bar only
   exists in a session started with this layout: a plain `zellij` with `default_layout "compact"` never
   loads the plugin. To make it your default, copy the layout to `~/.config/zellij/layouts/claude.kdl` and
   start with `zellij -n claude`, or set `default_layout "claude"` in `config.kdl`.

   **One-time permission prompt**: the first time, zellij asks in the bar whether the plugin may read
   and change the session state, run commands (the sound player, helper scripts) and receive pipes
   (`Allow? (y/n)`): focus the bar with a mouse click on it or `Alt+↓` (zellij's default move-focus key)
   and press `y`. (With `keybinds clear-defaults=true` there may be no `Alt+↓`, and the 1-row bar can be
   hard to click; the prompt text can also appear cut off on the left in a narrow window. Then use the
   pre-grant below.) The bar then shows the tabs and can no longer be focused; focus returns to your pane.
   zellij remembers the answer for exactly this wasm path (`permissions.kdl` in zellij's cache dir).
   Pressing `n` leaves the bar showing "permissions denied"; a new session asks again. If you
   cannot answer the prompt (or just want to skip it), pre-grant before starting the session:
   `sh ${CLAUDE_CONFIG_DIR:-~/.claude}/plugins/marketplaces/zellij-wc3cz/setup.sh --permissions` (writes
   zellij's `permissions.kdl` atomically for the wasm path of that copy).

The marketplace clone is only refreshed by `claude plugin marketplace update zellij-wc3cz`: until then the
wasm the layout loads can lag behind the hooks of a newer plugin version.

That is all: voice lines, agent counts and attention markers work once a `claude` in that session has
run a hook (the hook tells the plugin where the package is). Plugin options go into the `plugin` block
of the layout (see Options), e.g. `plugin location="file:..." { ding_when_focused false; }`.

**Colour icons (optional, Linux and macOS)**: without the WC3 icon font the bar shows initials badges. With it,
each character gets its portrait:

```sh
sh ${CLAUDE_CONFIG_DIR:-~/.claude}/plugins/marketplaces/zellij-wc3cz/fontconfig/install.sh   # --uninstall to undo
```

On macOS the same command installs two files, `~/Library/Fonts/wc3-icons-mac.ttf` (an Apple `sbix` build, family
`WC3 Icons Mac`, for kitty) and `wc3-icons-mac-ghostty.ttf` (family `WC3 Icons Wide`, for ghostty), drops the cached font flag and prints the line your terminal needs (kitty:
`symbol_map U+1FA70-U+1FAD6,U+1F7E0-U+1F7E6 WC3 Icons Mac`; ghostty: `font-codepoint-map`); it edits no terminal
config. Quit and reopen the terminal afterwards.

It writes three files that belong to this package (`~/.local/share/fonts/wc3-icons.ttf`,
`~/.local/share/fonts/wc3-icons-wide.ttf` and `~/.config/fontconfig/conf.d/60-wc3-icons.conf` (macOS: only the two files `~/Library/Fonts/wc3-icons-mac.ttf`, `wc3-icons-mac-ghostty.ttf`), creating
`~/.config/fontconfig/conf.d/` if missing),
runs `fc-cache` and drops the plugin's cached font flag; it edits no terminal config. Then close ALL
windows of your terminal and start it again (a running kitty keeps its font set). If you installed the
font after a hook already ran, the flag (`/tmp/zellij-$(id -u)/zellij-claude/font`, re-checked every 10
min) may still say 0 and the bar shows initials badges: delete that file and the next hook event re-checks.
The rules make fontconfig use "WC3 Icons" for the font's character icon codepoints (U+1FA70..U+1FAD6 minus
gaps, one per character), ahead of other emoji fonts; every other emoji and all text are unchanged. The
seven race fallback codepoints U+1F7E0..U+1F7E6 (the orange to brown circle emoji) are NOT overridden, so
the usual status-dot emoji keep their look; a character without an icon in the font then shows the system
circle emoji where the race glyph would be. Side effect: in every app that uses fontconfig the few emoji
at the character codepoints (ballet shoes, ringed planet, chair, hamsa, ...) show WC3 portraits.
Two variants are installed: `WC3 Icons` (bitmap 112x144, for tall cells such as kitty with 110% line
height) and `WC3 Icons Wide` (120x116, for cells about half as wide as tall, i.e. most fonts at 100% line
height). Every terminal then needs one line of its own config (nothing below is edited by the script; fully
restart the terminal afterwards, a running one keeps its fonts). Measured on Linux (Wayland) with Paper Mono,
kitty 0.49.2, ghostty 1.3.1, alacritty 0.17.0, foot 1.28.0 (the glyphs were compared with the font's
bitmaps; details in `assets/wc3-icons/README.md`):

| Terminal | Config | Result (icon vs the 2-cell box) |
|----------|--------|--------------------------------|
| kitty | `kitty.conf`: `symbol_map U+1FA70-U+1FAD6 WC3 Icons` (fontconfig alone is not enough, kitty prefers Noto Color Emoji) | font_size 18 + `modify_font cell_height 110%` (14x36 cells): 28x36 in 28x36, pixel exact. Other sizes: undistorted, up to 2-4 px smaller than the box. Verified |
| ghostty | config: `font-codepoint-map = U+1FA70-U+1FAD6=WC3 Icons Wide` (it ignores fontconfig for emoji) | ghostty fits the icon into 2 cells x the font's NATURAL line height, centred, at any size: font-size 18 = 30x29 in 30x29, exact. `adjust-cell-height = N` leaves N px of background above/below (N=3: 2 px above, 1 below). With `WC3 Icons` instead: 22x29 (4 px side gaps). Verified |
| foot | `foot.ini` `[main]`: `font=monospace:size=8,WC3 Icons:size=8` (the icon font as fallback, same size as the text font; needs the emoji tag that `install.sh` adds) | foot defaults (monospace 8 = 6x16 cells): 12x16 in 12x16 with `WC3 Icons`, exact. Paper Mono 18 (15x29): use `WC3 Icons Wide`, 30x29, exact. foot clips at the cell, so a wrong variant is cut off. Verified |
| alacritty | a fontconfig rule (alacritty has no fallback setting), e.g. `~/.config/fontconfig/conf.d/61-wc3-icons-alacritty.conf`: `<match target="pattern"><test name="prgname"><string>alacritty</string></test><test name="family"><string>YOUR FONT</string></test><edit name="family" mode="append" binding="same"><string>WC3 Icons Wide</string></edit></match>` inside `<fontconfig>` | alacritty draws the bitmap at font_px/96, not fitted to the cell, bottom anchored at the baseline: Paper Mono 18 with `font.offset.y = 0` (15x29): 30x29, exact. Other line heights overflow into the line above (`offset.y = -2`: 2 px) or leave a gap; build a `WC3 Icons Custom` font for that cell (`assets/wc3-icons/README.md`, e.g. 15x27: 30x27 exact) and name it in the rule. The rule also makes alacritty draw the race icons for the circle emoji U+1F7E0..U+1F7E6. Verified |
| wezterm, konsole, others | untested | |
| kitty (macOS) | CoreText draws no CBDT, so a separate sbix font (`WC3 Icons Mac`; the ghostty one below is a second file): `sh fontconfig/install.sh` (installs `~/Library/Fonts/wc3-icons-mac.ttf`) + `kitty.conf`: `symbol_map U+1FA70-U+1FAD6,U+1F7E0-U+1F7E6 WC3 Icons Mac` | Paper Mono 18 + `cell_height 110%` on Retina (22x47 px cells): 44x47 in 44x47, exact; stacked rows seamless. Other fonts/sizes: undistorted, may be smaller/clipped (rebuild recipe in `assets/wc3-icons/README.md`). Verified |
| ghostty (macOS) | same install; config: `font-codepoint-map = U+1FA70-U+1FAD6=WC3 Icons Wide` (+ the same line for `U+1F7E0-U+1F7E6`). The macOS sbix font uses the SAME family name as the Linux one, so a shared (dotfiles) config works on both | tuned for Paper Mono 21 + `adjust-cell-height = 4` (Retina cells 25x54 px): 50x50 px icon with 2 px of tab colour on every side; other sizes need a rebuild with another `SBIX_DX` (assets/wc3-icons/README.md); `adjust-cell-height = N` adds N px of background above/below. Verified |

A single bitmap cannot fit every cell: kitty and ghostty scale it to the cell, foot and alacritty draw it
at the font size, so exact fits hold only for the settings named above (other sizes: smaller icon or gaps of
1-4 px; never distorted).

A terminal that cannot draw colour bitmap fonts shows another emoji or a box instead: set
`race_font "false"` in the plugin block (initials badges) or run `fontconfig/install.sh --uninstall`.

macOS: the same two steps. Sounds play through `afplay` (FLAC needs macOS 10.13+).

## Uninstall

1. Bar: start zellij without the layout (or `default_layout "compact"`) and remove the pane from your own
   layouts.
2. Hooks: `claude plugin uninstall zellij-wc3cz@zellij-wc3cz`, then `claude plugin marketplace remove
   zellij-wc3cz` (run both once per Claude config dir you installed it in). Hooks that an older `setup.sh`
   wrote into `settings.json`: `sh <pkg>/setup.sh --migrate-to-plugin` (before deleting the checkout).
3. Icon font (and the `symbol_map` line you added to `kitty.conf`, if any): `sh <pkg>/fontconfig/install.sh --uninstall` (or `setup.sh --font-uninstall`) removes
   `~/.local/share/fonts/wc3-icons.ttf`, `wc3-icons-wide.ttf` (and a `WC3 Icons Custom` build saved as
   `wc3-icons-custom.ttf`) and `~/.config/fontconfig/conf.d/60-wc3-icons.conf`. While the font
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

- `--font` / `--font-uninstall`: `fontconfig/install.sh` (Linux; macOS: `install-mac.sh`).
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
| `max_clip_s` | `0` (no limit) | longest voice line to play, in seconds: longer clips are not picked, and a character with no shorter clip plays its shortest one. Some lines run up to about 25 s; set e.g. `8` if you find them annoying. A second ding that arrives while a long clip plays still waits for it (clip length + 5 s, at most 40 s). Fractional values are fine (`12.5`); junk or negative = no limit. The plugin passes it to `play-phrase.sh` as `ZELLIJ_CLAUDE_MAX_CLIP_S`; when the option is unset, a script run by hand also honours that variable or the first line of the file `zellij-claude/max_clip_s` in the zellij temp dir (`/tmp/zellij-<uid>/zellij-claude/max_clip_s` on Linux) |
| `dup_window_ms` | `2000` | a ding for the same character that follows another within this time is dropped (finish + question of one tab, two panes) |
| `ding_when_focused` | `false` | also ding when the finished tab is the active one |
| `ding_when_os_unfocused` | `true` | ding and blink the ACTIVE tab too when the terminal window was not the OS-focused window at the finish (see "Away from the terminal") |
| `ding_with_subagents` | `false` | ding on every Stop, even while subagents run |
| `subagent_grace_s` | `8` | wait after the last subagent stops before dinging (main agent may continue) |
| `subagent_stale_s` | `900` | ignore subagent markers older than this (missed SubagentStop; an agent stopped with TaskStop or killed is cleared at once / at the next Stop) |
| `working_hold_s` | `20` | any hook event keeps the working look this long |
| `auto_switch` | `false` | switch to a tab whose agent finished |
| `auto_name` | `true` | label tabs with the folder of their main pane, or `SSH: host` while an ssh session runs there (user-chosen names win) |
| `title_fallback` | `true` | detect Claude panes by command/title when there is no hook state |
| `tab_numbers` | `false` | prefix tab labels with their number |
| `clock` | `true` | `HH:MM` on the right |
| `clock_character` | `true` | name of the active tab's character (without any `(...)` suffix) left of the clock (needs `clock`; truncated, then dropped, when space is short) |
| `show_other_clients` | `false` | stock `[ ]` markers of other attached clients |
| `active_underline` | `false` | underline the active tab |
| `subagent_count` | `true` | number of running subagents after `✻` |
| `spinner` | `true` | animated `✻` while working (`false` = static, italic) |
| `spin_ms` | `160` | spinner step (60-2000) |
| `blink` | `true` | blink finished-unseen tabs |
| `blink_ms` | `900` | blink phase (200-5000) |
| `blink_timeout_s` | `600` | stop blinking after this (0 = never) |
| `race_icons` | `true` | show the character icon (colour font) or its initials badge |
| `race_font` | `auto` | colour WC3 icon font. `auto` = on when the hook found the `WC3 Icons` font installed (`fc-list`; on macOS the file `wc3-icons-mac.ttf` in `~/Library/Fonts` or `/Library/Fonts`; re-checked every 10 min), otherwise each tab shows a 2-cell initials badge (`AM` Arthas Menethill, `ČA` Čarodějka; unique for all 92 characters) on a chip of the character's colour, which works in any terminal. `true` forces the icon font, `false` forces the initials badges |
| `race_tab_colors` | `true` | tint tabs in the character's colour |
| `race_dim` | `0.4` | brightness of inactive tinted tabs (0.1-1.0) |
| `plain_dim` | `0.18` | brightness of inactive untinted tabs (plain tabs): the theme's ribbon colour blended toward the bar background (0.1-1.0, 1.0 = theme colour as is). The default gives a plain tab the same luminance as the median inactive Claude tab at `race_dim` 0.4 |
| `race_all_tabs` | `false` | give characters to tabs without Claude too |
| `diversity` | `true` | new characters are picked by weighted random against the session's tabs: unused characters only, then less likely when the race, hero/unit role, sex or voice theme matches an existing tab (twice as much for the adjacent tab; never the same race with the same role or theme next to each other), and when picked recently. `false` = uniform random among unused characters |
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
                           # (reloads only a plugin present in the session; otherwise refuses, use -p <exact location>)
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
  terminal (close all its windows); each terminal needs its line from the Quick start table (kitty
  `symbol_map`, ghostty `font-codepoint-map`, foot `font=` fallback, alacritty a fontconfig rule). A kitty
  `symbol_map` must cover U+1FA70-U+1FAD6 (a stale U+F0100 range gives grey blobs). Still initials badges
  after installing the font: the font flag is cached, `rm /tmp/zellij-$(id -u)/zellij-claude/font`.
  Terminals that cannot draw it: `race_font "false"` (initials badges). Icon too small, cut off or
  overlapping the line above: pick the other variant (`WC3 Icons` for tall cells, `WC3 Icons Wide` for
  ~1:2 cells) or build a `WC3 Icons Custom` for your cell (`assets/wc3-icons/README.md`).
- **New option has no effect**: options are fixed at session start; start a new session.
- **Old bar after an update**: sessions started before keep their plugin; `dev.sh -s SESSION` reloads
  the code, a new session picks up everything.
- **Permission prompt every time / cannot answer it**: zellij keys the grant by the exact (`~`-expanded)
  wasm path in the layout, so a moved checkout or another path asks again. The prompt is answered in the
  bar: click it (or `Alt+↓`, which may not exist with `clear-defaults=true`), then `y`. `<pkg>/setup.sh --permissions` pre-grants this package's path (run
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

## macOS checklist

Run in a zellij pane on the Mac and check the result:

```sh
zellij --version                                     # 0.45.x
sh <pkg>/setup.sh     # status only; "WC3 icon font: not installed" until `fontconfig/install.sh` ran
printf '%s\n' "$TMPDIR"; ls "${TMPDIR%/}"/zellij-$(id -u)/   # must contain zellij-log
sh <pkg>/claude-hook.sh start; ls "${TMPDIR%/}"/zellij-$(id -u)/zellij-claude/*/   # a file named like the pane id
afplay <pkg>/sounds/ding.wav
afplay "$(ls <pkg>/sounds/wc3/human/*.flac | head -n 1)"   # FLAC through CoreAudio
<pkg>/play-phrase.sh --list "" | wc -l      # 91 characters
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
