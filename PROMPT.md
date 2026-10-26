# Install guide for the installing agent

You are a coding agent installing this package (a zellij 0.45 tab bar with Claude Code awareness and Warcraft III
characters/voices) for the user. This file is your runbook. Read it fully, then work through the steps in order.
`<pkg>` = the package dir (the checkout or `${CLAUDE_CONFIG_DIR:-~/.claude}/plugins/marketplaces/zellij-wc3cz`).
Background and full option list: `docs/REFERENCE.md`; internals: `AGENTS.md`.

## 0. Principles

- Change no user config silently. Before editing the user's zellij config/layouts, terminal config or Claude
  `settings.json`: show the exact change, get a yes, back the file up (`cp -p FILE FILE.bak-wc3cz`).
- The package's own scripts edit nothing by default (`sh <pkg>/setup.sh` without options = read-only status).
  Only the `claude plugin ...` commands touch Claude settings (they add `extraKnownMarketplaces`/`enabledPlugins`).
- Never print secrets/tokens. Kill processes by exact PID, never by name. Never inject keystrokes (`wtype`,
  `xdotool type`, `tmux send-keys` into the user's terminal). Do not attach to, reload or kill the user's running
  zellij sessions.
- Any test runs in an ISOLATED session: private `HOME`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`, `XDG_DATA_HOME`,
  `ZELLIJ_CONFIG_DIR`, `CLAUDE_CONFIG_DIR` in a scratch dir, and a differently named session. Never run
  installers meant for the real HOME against a scratch HOME by mistake, or the reverse.
- Do not write real hostnames, accounts or home paths into repo files. Do not add AI attribution anywhere.
- Say what you could not verify (see step 8).

## 1. Detect the environment (read-only)

```sh
uname -s; uname -m
zellij --version                       # must be 0.45.x; otherwise stop and tell the user
claude --version                       # needs plugin support (claude plugin --help works)
echo "CLAUDE_CONFIG_DIR=${CLAUDE_CONFIG_DIR:-}"; ls -d ~/.claude* 2>/dev/null
command -v pw-play paplay ffplay aplay afplay     # sound player (afplay built in on macOS)
command -v fc-list ss jq                          # optional helpers
```

- **Claude config dirs**: ask whether other dirs (e.g. a work profile via `CLAUDE_CONFIG_DIR`) should get the
  plugin too. Each dir needs its own marketplace add + install (step 3). One zellij layout serves all.
- **Terminal** (needed for icons, step 6). Check `echo $KITTY_WINDOW_ID`, `echo $GHOSTTY_RESOURCES_DIR`,
  `echo $TERM_PROGRAM`, `echo $TERM`, `echo $ALACRITTY_SOCKET`, `echo $WEZTERM_EXECUTABLE`, and the process
  ancestry (`ps -o comm= -p $PPID`, walk up with `ps -o ppid=`). If you run inside zellij the env still holds
  the host terminal's variables. If unsure, ask the user.
- **Existing zellij setup**: `ls ~/.config/zellij ~/.config/zellij/layouts 2>/dev/null` (honour
  `$ZELLIJ_CONFIG_DIR`), `grep -n 'default_layout\|claude-tabs' ~/.config/zellij/config.kdl`. A pre-existing
  `claude-tabs` alias or layout means an older install: keep its wasm path stable (permission grants are keyed by
  it) and ask before changing it.
- **Old install**: `sh <pkg>/setup.sh` (after the clone in step 2, or from an existing checkout). It reports per
  Claude dir whether the plugin is installed and whether old `claude-hook.sh` hooks sit in `settings.json`
  ("DOUBLE REGISTRATION" = every hook runs twice). Fix with step 3b.
- **Rust**: NOT needed. `claude-tabs.wasm` is prebuilt and tracked. Only `setup.sh --build` needs Rust.
- **Disk**: about 270 MB for the clone plus about 140 MB per cached plugin version (the sound library). Tell the
  user before downloading.

## 2. Get the package

Either route; the plugin route (3a) already clones it to `<config dir>/plugins/marketplaces/zellij-wc3cz/`, which
is what the shipped layout loads. For a standalone checkout (development, or no plugin route):

```sh
git clone https://github.com/crossatko/zellij-wc3cz-claude ~/code/zellij-wc3cz-claude   # any location
```

If the user already has a checkout, use it (`claude plugin marketplace add <path>` loads a local dir in place; the
layout `location` must then point at that checkout's `claude-tabs.wasm`).

## 3. Hooks: the Claude Code plugin (once per config dir)

3a. Install (confirm the download size first; the add takes about a minute):

```sh
claude plugin marketplace add crossatko/zellij-wc3cz-claude
claude plugin install zellij-wc3cz@zellij-wc3cz
# another config dir:
CLAUDE_CONFIG_DIR=~/.claude-work claude plugin marketplace add crossatko/zellij-wc3cz-claude
CLAUDE_CONFIG_DIR=~/.claude-work claude plugin install zellij-wc3cz@zellij-wc3cz
claude plugin list                     # zellij-wc3cz enabled
```

The clone follows the user's git config (HTTPS, or SSH if GitHub URLs are rewritten to SSH, which needs a key).
Running `claude` sessions must be restarted (or `/reload-plugins`) to load the hooks; tell the user.
`claude plugin validate <pkg>` may warn about a missing `version`: that is intended.

3b. Old settings hooks: if `sh <pkg>/setup.sh` shows `old claude-hook.sh hooks in settings.json: yes` (or
DOUBLE REGISTRATION), ask, then migrate. It needs `jq`, backs up to `<file>.bak-claude-tabs`, removes only our
handlers and is idempotent:

```sh
sh <pkg>/setup.sh --migrate-to-plugin                     # default settings file
sh <pkg>/setup.sh --migrate-to-plugin --settings ~/.claude-work/settings.json   # repeatable, or --all-claude-dirs
```

Never use `--legacy-settings-hooks` (deprecated) or `--link` unless the user's config already uses the old alias
path `file:~/.config/zellij/plugins/claude-tabs/claude-tabs.wasm`.

## 4. The bar: a zellij layout

zellij only loads plugins named in a layout or config. A plain `zellij` with `default_layout "compact"` never
loads this one. Pick with the user:

- **Try it, no config edits**: `zellij -n ${CLAUDE_CONFIG_DIR:-~/.claude}/plugins/marketplaces/zellij-wc3cz/layouts/claude.kdl`
  (`-n` = new session; `--layout` inside a session ADDS tabs instead). The shipped layout loads
  `file:~/.claude/plugins/marketplaces/zellij-wc3cz/claude-tabs.wasm`. KDL does not expand variables, so for a
  non-default `CLAUDE_CONFIG_DIR` the file needs the real path: copy the layout (below) and edit `location=`.
  Hand the command to the user to run; do not launch their terminal sessions for them.
- **Make it the default**: with consent, `cp <pkg>/layouts/claude.kdl ~/.config/zellij/layouts/claude.kdl`
  (`$ZELLIJ_CONFIG_DIR/layouts/`), then `zellij -n claude`, or add `default_layout "claude"` to `config.kdl`.
- **Existing custom layout**: replace its `compact-bar`/`tab-bar` pane with
  ```kdl
  pane size=1 borderless=true {
      plugin location="file:~/.claude/plugins/marketplaces/zellij-wc3cz/claude-tabs.wasm"
  }
  ```
  Show the diff and get a yes first. A `claude-tabs` alias in `config.kdl` also works if the user already has one.
- Plugin options go inside the `plugin { ... }` block (step 7). They are fixed at session start.
- The marketplace clone (and so the wasm) only updates with `claude plugin marketplace update zellij-wc3cz`.

## 5. Permissions

On the first session zellij asks in the bar ("... Allow? (y/n)"): the user clicks the bar or presses `Alt+Down`,
then `y`. It is remembered per exact wasm path. To skip or when the bar cannot be focused (`keybinds
clear-defaults=true`, narrow window), pre-grant, with the user's OK, using the `setup.sh` of the copy the layout
points at:

```sh
sh ${CLAUDE_CONFIG_DIR:-~/.claude}/plugins/marketplaces/zellij-wc3cz/setup.sh --permissions
```

It rewrites zellij's `permissions.kdl` atomically (Linux `${XDG_CACHE_HOME:-~/.cache}/zellij/`, macOS
`~/Library/Caches/org.Zellij-Contributors.Zellij/`) and keeps other content. If the user answered `n`, the bar
shows "permissions denied": start a new session and answer `y`, or pre-grant.

## 6. Colour icons (optional; ask)

Without the icon font the bar shows 2-letter initials badges, which work in any terminal. Ask whether the user
wants the portraits. Mention side effect (Linux): the few emoji at those codepoints (ballet shoes, ringed planet,
...) show WC3 portraits in every fontconfig app until uninstalled.

Preview, read-only: `sh <pkg>/fontconfig/install.sh --check` (Linux) / `install-mac.sh --check`.

**Linux** (installs 2 fonts + `60-wc3-icons.conf`, runs `fc-cache`, edits no terminal config):

```sh
sh <pkg>/fontconfig/install.sh
```

Then ONE terminal config edit (ask; show the diff; both codepoint ranges, `U+1F7E0-U+1F7E6` are the race
fallbacks). Which variant depends on cell shape: `WC3 Icons` for tall cells (kitty with 110% line height),
`WC3 Icons Wide` for cells about 1:2.

| Terminal | Edit |
|---|---|
| kitty | `kitty.conf`: `symbol_map U+1FA70-U+1FAD6 WC3 Icons` (required, fontconfig alone is not enough). Exact at font_size 18 + `modify_font cell_height 110%`; other sizes are undistorted but a few px smaller |
| ghostty | config: `font-codepoint-map = U+1FA70-U+1FAD6=WC3 Icons Wide` (ghostty ignores fontconfig for emoji). `adjust-cell-height = N` leaves N px of background above/below the icon |
| foot | `foot.ini` `[main]`: `font=<your font>:size=N,WC3 Icons:size=N` (icon font as fallback, same size; for cells about 1:2 use `WC3 Icons Wide`). A wrong variant is clipped |
| alacritty | no fallback setting: add a fontconfig rule, e.g. `~/.config/fontconfig/conf.d/61-wc3-icons-alacritty.conf` with `<match target="pattern"><test name="prgname"><string>alacritty</string></test><test name="family"><string>YOUR FONT</string></test><edit name="family" mode="append" binding="same"><string>WC3 Icons Wide</string></edit></match>` inside `<fontconfig>`; line heights other than the default may need a custom build (`assets/wc3-icons/README.md`) |
| wezterm, konsole, gnome-terminal, others | untested. Offer initials badges (`race_font "false"` in the plugin block) or try it and ask the user to look |

**macOS** (CoreText cannot draw the Linux font, so `install.sh` hands over to `install-mac.sh`, which installs
`~/Library/Fonts/wc3-icons-mac.ttf` and `wc3-icons-mac-ghostty.ttf`; no terminal config edited):

```sh
sh <pkg>/fontconfig/install.sh        # -> install-mac.sh
```

kitty: `symbol_map U+1FA70-U+1FAD6,U+1F7E0-U+1F7E6 WC3 Icons Mac`. ghostty: `font-codepoint-map =
U+1FA70-U+1FAD6=WC3 Icons Wide` and the same line for `U+1F7E0-U+1F7E6` (the last matching line wins even if the
family is missing). The ghostty font is tuned for Paper Mono 21 + `adjust-cell-height = 4`; other sizes may show
gaps/clipping (recipe in `assets/wc3-icons/README.md`). Other macOS terminals: initials badges.

After editing the terminal config the user must close ALL windows of the terminal and start it again (a running
one keeps its fonts). If the bar still shows initials, the plugin's cached font flag is stale: delete
`/tmp/zellij-$(id -u)/zellij-claude/font` (macOS: under `$TMPDIR`); the next hook event re-checks.
The flag only says "installed", not "this terminal draws it". If icons come out as boxes or wrong emoji, set
`race_font "false"` or run `install.sh --uninstall`.

**Sound** (voice lines are CZECH ONLY; there is no English version): confirm a player exists (step 1). macOS
uses `afplay`. If none, install `pipewire` (`pw-play`) / `pulseaudio-utils` (`paplay`) or ffmpeg, or the plugin
falls back silently. Offer `max_clip_s` (step 7) since some lines run up to about 25 s.

## 7. Options worth asking about (plugin block of the layout; ask, don't assume)

```kdl
plugin location="file:..." {
    ding_when_focused false     // true = also ding for the tab you are looking at
    clock_character true        // character name next to the clock (default on)
    race_dim "0.4"              // 0.1-1.0 brightness of inactive coloured tabs
    max_clip_s "8"              // skip voice lines longer than this many seconds (default: no limit)
    lead_in_ms "300"            // silence before a clip; for Bluetooth speakers that clip the first ms
    race_font "false"           // force initials badges (terminal cannot draw the icon font)
    auto_switch false           // jump to a tab when its agent finishes
}
```

Everything else (characters/races pool, attention alerts, subagent timing, spinner/blink, `package_dir`, ...) is in
the Options table of `docs/REFERENCE.md`. Options are read at session start only.

## 8. Verify (be honest about what you cannot see)

Do not touch the user's live sessions. For checks that need a running session, use an isolated one (scratch
`HOME`/`XDG_*`/`ZELLIJ_CONFIG_DIR`, own session name, a copy of the wasm if you test a build) and a stub player if
you do not want sound; or ask the user to run the check in their own pane.

Read-only checks you can run directly:

```sh
sh -n <pkg>/claude-hook.sh && sh -n <pkg>/setup.sh                 # scripts parse
sh <pkg>/setup.sh                                                  # plugin installed: yes; no old hooks; font status
claude plugin list                                                 # zellij-wc3cz enabled
<pkg>/play-phrase.sh --list "" | wc -l                             # 91 characters found
sh <pkg>/fontconfig/install.sh --check                             # Linux: WC3 Icons expected for charset=1fa70
```

Checks that need a zellij session (hand to the user or isolate): inside a pane `<pkg>/claude-ctl.sh ding "Thrall"`
plays a voice line (else try the player by hand, e.g. `pw-play <pkg>/sounds/ding.wav`); in a session with the
bar, start `claude`, then `grep claude-tabs /tmp/zellij-$(id -u)/zellij-log/zellij.log` shows `scan found [...]`
and `tab N -> character`; `/tmp/zellij-$(id -u)/zellij-claude/ding.log` has one line per ding. Isolated hook
simulation: `ZELLIJ=0 ZELLIJ_SESSION_NAME=<test> ZELLIJ_PANE_ID=1 sh <pkg>/claude-hook.sh done </dev/null` writes
`<state root>/zellij-claude/<test>/1`.

You cannot verify: how the bar and icons LOOK in the user's terminal (ask them to look; if you take a screenshot
of an isolated test window, crop only that window, never the whole screen), the permission prompt flow, macOS
runtime (hooks, `afplay`, OS-focus helper), terminals other than kitty/ghostty/foot/alacritty. Say so in the report.

## 9. Troubleshooting

| Symptom | Cause / fix |
|---|---|
| No bar at all | session not started with the layout; plain `zellij` + `default_layout "compact"` never loads it. Check the `location=` path exists (non-default `CLAUDE_CONFIG_DIR`!) |
| Bar says "permissions denied" | answered `n`: new session + `y`, or `setup.sh --permissions` |
| Permission prompt every session | grant is keyed by the exact `~`-expanded wasm path; a different path asks again. Pre-grant for the path the layout uses |
| Initials instead of icons | font not installed, terminal line missing, terminal not fully restarted, or stale font flag (`rm .../zellij-claude/font`). Or `race_font "false"` was set |
| Boxes / wrong emoji / clipped icon | terminal cannot draw it or wrong variant (`WC3 Icons` vs `Wide`); kitty needs `symbol_map`; set `race_font "false"` |
| No tab character / voice, bar otherwise fine | hooks not active: plugin installed in that config dir? restart `claude`; plugin learns `<pkg>` from the first hook event; or set `package_dir` in the plugin block |
| Doubled dings / hooks | plugin + old settings hooks: `sh <pkg>/setup.sh` shows DOUBLE REGISTRATION; run `--migrate-to-plugin` |
| Never hear a ding | no ding for the active tab while the terminal has OS focus (`ding_when_focused`) and none while subagents run; check the player; `ding.log` |
| Clip start cut off (Bluetooth) | `lead_in_ms "300"`, restart session |
| Option has no effect | options are fixed at session start: new session |
| Bar older than hooks after update | `claude plugin marketplace update zellij-wc3cz`, then new session |
| macOS | hooks/afplay/OS-focus runtime not verified; run the "macOS checklist" in `docs/REFERENCE.md` and report differences. `$TMPDIR` replaces `/tmp` for state files |

## 10. Final report to the user

List: files you changed or created (with backups), commands that changed state (`claude plugin ...`,
`setup.sh --migrate-to-plugin/--permissions`, `install.sh`), what the user still has to do (restart `claude`
sessions, fully restart the terminal, start zellij with the layout, answer `y` once), what you did NOT verify
(step 8), and how to undo:

```sh
claude plugin uninstall zellij-wc3cz@zellij-wc3cz && claude plugin marketplace remove zellij-wc3cz   # per config dir
sh <pkg>/fontconfig/install.sh --uninstall        # or setup.sh --font-uninstall; also remove the terminal font line
# remove the claude-tabs pane / layout file / default_layout line you added; restore *.bak-wc3cz backups
# permission grant: delete the block named after claude-tabs.wasm in zellij's permissions.kdl
rm -r /tmp/zellij-$(id -u)/zellij-claude          # runtime state (macOS: under $TMPDIR)
```

Full uninstall list: "Uninstall" in `docs/REFERENCE.md`.
