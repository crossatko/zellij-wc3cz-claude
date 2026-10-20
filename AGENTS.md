# claude-tabs (zellij-wc3cz-claude): notes for agents

State and history of this package, for whoever works on it next. User-facing docs: `README.md`.
Keep entries short and factual, update in place, no secrets. The owner commits/pushes; agents do not.

## What it is
A zellij 0.45 tab-bar plugin (Rust -> wasm32-wasip1, zellij-tile 0.45.1) that replaces the stock
`compact-bar` (ported verbatim from zellij v0.45.1 `default-plugins/compact-bar`: `line.rs`, `tab.rs`,
clipboard hints; tooltips dropped) and adds Claude Code awareness: `✳` marker + Claude Code's spinner
while working, running-subagent count (`✳2`), finished-unseen badge + blink, a Warcraft III "race" per
Claude tab (icon + tint), a Czech WC3 voice line of that race when an agent finishes (fallback
`sounds/ding.wav`), cwd-based tab labels, clock. Linux verified; macOS written blind (see below).

## Layout of this dir (standalone; formerly part of the owner's dotfiles)
- `src/main.rs` (state, races, sounds, hooks, timers), `src/tab.rs` (ribbon render, tint), `src/line.rs`
  (bar layout, ported), `src/clipboard_utils.rs`; `Cargo.toml`/`Cargo.lock`.
- `claude-tabs.wasm`: TRACKED prebuilt (what configs load; rebuild + commit it after every source change).
  `target/` is git-ignored.
- `setup.sh` (install/update), `dev.sh` (build + hot reload).
- Shell side (POSIX sh, bash 3.2 / BSD-safe, all locate siblings via `$(dirname "$0")`):
  `claude-hook.sh` (Claude Code hook), `claude-agents.sh` (running agents from transcripts),
  `claude-ctl.sh` (ding/race/reshuffle via `zellij pipe`), `play-phrase.sh` (clip picker, `--list`,
  `--ding`), `os-focus.sh` (OS window focus), `claude-lib.sh` (sourced helpers, mode 644).
- `layouts/claude.kdl` (example layout), `sounds/ding.wav`, `sounds/wc3/` (library),
  `assets/wc3-icons/` (icon font + build). `.gitattributes`: flac/wav/wasm/ttf/webp/png binary (no LFS).

## Path model (2026-10-20)
- The wasm cannot learn where it was loaded from. Option `package_dir` (default
  `~/.config/zellij/plugins/claude-tabs`) is where it finds `play-phrase.sh` (`phrase_script` default
  `<package_dir>/play-phrase.sh`), `claude-agents.sh` (next to `phrase_script`, or `agents_script`) and,
  through play-phrase.sh, `sounds/` (empty `sounds_dir` = `sounds/wc3` next to the script).
- `setup.sh` links `${ZELLIJ_CONFIG_DIR:-${XDG_CONFIG_HOME:-~/.config}/zellij}/plugins/claude-tabs` to
  the checkout (leaves an existing different path alone), so the example alias
  `file:~/.config/zellij/plugins/claude-tabs/claude-tabs.wasm` and the default `package_dir` work.
- Permission grants are keyed by the exact alias path (~ expanded, symlink NOT resolved): setup.sh
  grants both the link path and the physical checkout path. Linux cache
  `${XDG_CACHE_HOME:-~/.cache}/zellij/permissions.kdl`, macOS
  `~/Library/Caches/org.Zellij-Contributors.Zellij/permissions.kdl`.
- Hooks: setup.sh writes the ABSOLUTE physical path of `claude-hook.sh` into each target settings file,
  repoints existing `claude-hook.sh` commands from older locations, adds missing events; per-file backup
  `<file>.bak-claude-tabs` only when changed; needs jq; `--no-hooks` skips. Targets: `--settings FILE`
  (repeatable) and/or `--all-claude-dirs` (each `~/.claude*` DIRECTORY, `~/.claude.json` is ignored);
  without them `$CLAUDE_SETTINGS`, else `$CLAUDE_CONFIG_DIR/settings.json`, else
  `~/.claude/settings.json`. Missing dir = skipped, nothing created; an empty/missing file in an existing
  dir is created; invalid JSON = error, file untouched, rc 1. Verified on a fake HOME (2026-10-20): all
  of these cases, rerun = byte-identical, other hooks/keys kept.
- Multiple Claude config dirs need nothing at runtime: the hook takes `transcript_path` from the payload,
  the plugin and `claude-agents.sh` only use the dir the hook recorded; nothing else references
  `~/.claude` (grep of src/ and scripts).

## Build / dev / test
- Toolchain: Arch `sudo pacman -S rust rust-wasm`; macOS `brew install rustup && rustup-init -y &&
  rustup target add wasm32-wasip1`. Build: `cargo build --release --target wasm32-wasip1`, then copy
  `target/wasm32-wasip1/release/claude-tabs.wasm` over `claude-tabs.wasm` (`dev.sh`/`setup.sh` do this).
- Hot reload: `./dev.sh [-s SESSION | --all] [--watch]` = build, copy, `zellij --session S action
  start-or-reload-plugin claude-tabs` (alias name reloads every per-tab instance; no prompt; hook state
  kept). A reload picks up new wasm, NOT new alias options (those are fixed at session start).
  `--watch`: inotifywait, fswatch, entr, else 1 s polling.
- Test safely in an ISOLATED session (e.g. `pktest`) with its own `ZELLIJ_CONFIG_DIR`/`XDG_CACHE_HOME`,
  a test wasm copy and a stub player; drive it with simulated hook calls
  (`ZELLIJ=0 ZELLIJ_SESSION_NAME=pktest ZELLIJ_PANE_ID=N sh claude-hook.sh done </dev/null`). Never kill or
  reshuffle the owner's live sessions; no screenshots of the screen (text/SGR dumps only).
- Logs: `grep claude-tabs /tmp/zellij-$(id -u)/zellij-log/zellij.log` (`scan found [...]`,
  `tab N -> race`, `ding_command failed`). Hook debug: `touch <state root>/debug` logs every event + JSON
  to `<state root>/debug.log`.
- Do not `zellij pipe` this plugin bare: it blocks until every per-tab instance answers; `claude-ctl.sh`
  caps it with a timeout and sends the payload on stdin (empty payload at EOF is ignored by `claude_ding`).
- Sanity: all scripts pass `sh -n` and `bash --posix -n`.

## How it works (findings)
- State root = the dir the plugin sees as `/tmp` = `<temp_dir()>/zellij-<uid>` (zellij-utils consts.rs,
  plugin_loader.rs preopens `/tmp`); `claude_state_root` picks `$TMPDIR`, `/tmp` or
  `getconf DARWIN_USER_TEMP_DIR`, the first containing `zellij-<uid>/zellij-log`. Files under
  `zellij-claude/<session>/`: `<pane>` (`idle|working|done <ns> [sub]`), `<pane>.sub/<agent_id>` markers,
  `<pane>.ev` (activity), `<pane>.sess` (agent transcript dir), `races/<tab_id>`, `names/<tab_id>`;
  `last-<race>` (last clip).
- Hooks: SessionStart start, UserPromptSubmit working, PreToolUse tool, Stop done, SubagentStart
  substart, SubagentStop substop, SessionEnd end. Silent no-op outside zellij. Poke =
  backgrounded `zellij pipe --name claude_poke` (logs a harmless `CliPipe did not complete within 1s`);
  state is polled anyway (1 s). Without hook state a pane counts as Claude by command/title
  (`title_fallback`).
- One instance per tab; instances in INACTIVE tabs get no Tab/PaneUpdate/Visible events (stale data;
  after a reload also no ModeUpdate, so the session name is learned from `get_session_list()`). Only the
  LEADER (lowest live claude-tabs plugin pane id in a fresh `get_session_list()` snapshot) dings,
  auto-switches, acknowledges Done -> Idle, assigns races and deletes state of closed panes. `perms`
  defaults to true (PermissionRequestResult is not re-sent after a reload).
- Focused tab: `get_focused_pane_info()` returns the tab ID while the manifest is keyed by POSITION;
  mapped through `TabInfo` (bug: spinner never animated after a tab was closed). Only the focused
  instance runs the fast timer (spinner, blink) and polls agents every 3 s.
- Running agents = UNION of hook markers and `claude-agents.sh <transcript>/subagents` (agent-<id>.jsonl
  modified < 10 min whose last line lacks `"stop_reason":"end_turn"`; newest 40 of last 30 min). Real data
  showed: a hand-back arrives as SubagentStop only, the main thread then runs turns without
  UserPromptSubmit (hence the `tool` hook), SubagentStart is unreliable.
- Ding only on a real finish: a Stop while agents run is `done <ns> sub` (still Working); after the last
  one stops wait `subagent_grace_s` (8) for the main agent to continue, then ding. Markers older than
  `subagent_stale_s` (900, hook env `ZELLIJ_CLAUDE_STALE_S`) are ignored; markers > 6 h dropped.
  `working_hold_s` (20): any hook event newer than the last state change keeps the working look.
  No ding for the ACTIVE tab unless `ding_when_focused` or the finish was `away` (OS focus section). `ding_with_subagents` = ding on every Stop.
- Races: pool = `races` option, else library folders with audio PLUS all icon races (7), least-used first,
  random among ties, clips preferred; stored per stable `tab_id` (`create_new`, first writer wins);
  `claude-ctl.sh race X` / `reshuffle`. Library scanned via `play-phrase.sh --list` every 30 s (wasm
  cannot read `$HOME`). A race without clips plays another race's clip; empty library = ding fallback
  (exit 3). Glyph/colour table `RACES` in `src/main.rs` (4th field = icon font codepoint); unknown
  folder names get `◆` with a hashed hue.
- Colours (`tint_colors`, `src/tab.rs`): inactive = accent scaled to `race_dim` (0.1-1, default 0.4),
  text mixed to just 4.5:1; active = accent +15%, black/white text, bold. Theme-independent.
  Working = italic marker, finished-unseen = inverted badge, blink phase `(ms / blink_ms) % 2`,
  `blink_timeout_s` (600). Spinner frames `· ✢ ✳ ✶ ✻ ✽` ping-pong from Claude Code 2.1.303.
- Tab labels: folder of the pane's ROOT process (`get_pane_cwd`, 2 s debounce); zellij's own auto-names
  follow the FOREGROUND process (Claude's Bash `cd /tmp && ...` renamed tabs to `tmp`), so they are
  ignored with `auto_name`. User names (changed during/within 3 s of interactive RenameTab) stick, stored
  in `names/`; `zellij action rename-tab` is not detected as a user name.
- `[ ]` other-clients cursors only with `show_other_clients`; short-lived `zellij action` CLI clients
  used to flash it.
- compact-bar parity verified in an isolated session (overflow, fullscreen, sync, narrow widths, mouse);
  not verified: non-NORMAL mode colours, bell markers, multi-client, nested sessions, clipboard hints.

## OS focus ("away", 2026-10-20)
- zellij-tile 0.45 has NO plugin event for terminal/OS focus (checked the `Event` enum in
  zellij-utils 0.45.1 `src/data.rs`; `FocusGained/FocusLost` exist only in the nested-session
  protocol), so `os-focus.sh [SESSION]` asks the desktop: focused window pid+title (macOS lsappinfo /
  osascript, Hyprland hyprctl, Sway swaymsg+jq, KDE Wayland kdotool, X11 xdotool/xprop; X11 tools are
  never trusted on Wayland; GNOME Wayland = unknown). Decision: title `<session> | ...` = focused
  (zellij 0.45 titles the host terminal like that; seen on this machine: `profound-newt | ✳ Claude Code`);
  else focused pid is an ancestor of a client of the session (Linux: `ss -xpH`, server-side sockets with
  local path `.../contract_version_1/<session>` give the client socket inode as peer -> client pid;
  else every `zellij` client process from ps) and the title does not name another running session.
  Every external call capped at 1 s (`run_timeout`); measured 10-50 ms here.
- Hook: Stop writes `done <ns>[ sub][ away]` (`away` only for a clear `unfocused`; env
  `ZELLIJ_CLAUDE_OS_FOCUS=0` skips). Old wasm builds ignore the extra token.
- Plugin (`ding_when_os_unfocused`, default true): an `away` finish on the ACTIVE tab dings, is not
  acknowledged and blinks (`tab_away`, `tab_blink`, `look_active` now follows the blink phase for the
  active tab too); the leader polls `os-focus.sh <session>` every 2 s (`ask_os_focus`, ctx `osfocus`)
  and acknowledges once a result other than `unfocused` arrives after the finish (`os_focus_ms` vs ns).
- Verified (isolated session `pktest`, server env with a file-controlled stub `hyprctl`, logging stub
  player, headless pty client via `script` + idle fifo, no keystrokes, no screenshots): away finish ->
  1 clip, stays `done ... away`, active tab bg alternates 170,128,220 / 62,42,86; stub title switched
  to `pktest | ...` -> acknowledged within 4 s, steady bright; focused finish -> no clip, idle; no backend
  (unknown) -> no clip, idle. NOTE a session with no client has no active tab, so every finish there
  dings (old behaviour too). Real Hyprland read-only: session in the focused kitty -> focused via title,
  detached session -> unfocused. Helper unit-tested with PATH stubs for every backend (17 cases incl.
  title with comma/quote, same-pid other-session title, X11 no active window, macOS ps fallback, hung
  backend -> unknown after 1 s).
- NOT verified: real macOS, Sway, X11, KDE, GNOME; terminals other than kitty (alacritty/wezterm/ghostty
  rely on the same title + pid rules; Terminal.app/iTerm2 pid only); `ding_when_os_unfocused false`
  path (simple guard, not run).

## Sounds
- `sounds/wc3/<race>/*.flac`: 800 active clips (human 177, orc 159, undead 117, nightelf 133, naga 38,
  demon 18, neutral 158), `_excluded/<race>/` 75 (never played; `_`/hidden folders are never races),
  `manifest.tsv` (file, status, exclusion_reason, race, character, transcript, duration, video, times,
  lufs, flags), README, NOTICE (Blizzard content, included at the owner's decision). FLAC level 8, mono
  48 kHz s16, -20 LUFS, TP <= -2 dBTP; ~138 MB total, largest file 0.4 MB, names NFC.
- Masters and the whole production pipeline are OUTSIDE this repo in `~/Music/wc3-cz/` (yt-dlp of
  playlist PLQNF-ZjhDFeDZaqd5qCr6dE60e1XXgNnP, demucs vocals, GPU whisper.cpp Vulkan in
  `~/.local/share/wc3-gpu`, LLM-planned cuts in `scripts/opus/`, validation in `work/validation/`).
  Never commit masters/raw/vocals. Re-package after library changes:
  `python3 ~/Music/wc3-cz/scripts/package_dotfiles.py --dry` then without `--dry`; its target path still
  points at the OLD dotfiles location (`config/zellij/sounds/wc3`) and must be updated to this
  checkout's `sounds/wc3` first (not done). A real run rebuilds `_excluded` from its lists, undoing
  manual moves: diff against manifest.tsv first.
- Exclusions are heuristic (nothing was listened to); prune by moving files between `<race>/` and
  `_excluded/<race>/`. Transcripts in names are best effort; the 9 human `Rytíř (nejistý)` clips are
  probably Death Knight lines.
- Players: autodetect afplay, pw-play, paplay, ffplay, aplay; extensions per player (`claude_exts`:
  afplay no Ogg, pw-play/paplay no m4a, aplay wav only).
- `sounds/ding.wav`: derived from a downloaded chime `stu9-chime-356831.mp3` (not in the repo, licence
  unknown): `ffmpeg -y -i stu9-chime-356831.mp3 -filter_complex "[0:a]atrim=0:1.7,asetpts=PTS-STARTPTS,acrossover=split=3000:order=4th[lo][hi];[hi]afade=t=out:st=0.1:d=0.3:curve=qsin[hf];[lo][hf]amix=inputs=2:normalize=0,afade=t=in:d=0.005,afade=t=out:st=0.7:d=1.0:curve=cub,volume=6dB,alimiter=limit=0.66:attack=2:release=50:level=disabled" -ar 44100 -ac 1 -c:a pcm_s16le sounds/ding.wav`
  (no afftdn: it adds a low swoosh).

## Icon font (`assets/wc3-icons/`)
- `wc3-icons.ttf`: CBDT colour bitmap font, family `WC3 Icons`, U+1F7E0..U+1F7E6 (human, orc, undead,
  nightelf, naga, demon, neutral), 2 cells wide. Installed on Linux by setup.sh to
  `~/.local/share/fonts`; needs kitty `symbol_map U+1F7E0-U+1F7E6 WC3 Icons`. `race_font "auto"` = on
  for Linux, off for macOS (CoreText has no CBDT; an sbix variant would be needed).
- Rebuild: `cd assets/wc3-icons && FAMILY="WC3 Icons" PPEM=97 BY=116 ASPECT=0.7778 python3 build.py
  wc3-icons.ttf 1212 4 145` (= defaults). Tuned for kitty font_size 18 / cell_height 110% (cells 14x36):
  kitty draws colour bitmaps at the main font size scaled 24/PPEM and only shrinks in whole px, so the
  bitmap height *24/PPEM must stay just BELOW the cell height. Test builds need a different family name
  (an installed font with the same family silently wins). Old tuning workspace (outside the repo):
  `~/.local/share/wc3-icons/`.

## macOS (written without a Mac; unverified)
- Done for it: claude-lib.sh (Homebrew PATH, state-root candidates, `run_timeout` timeout/gtimeout/perl,
  `now_ns` without `%N`), no GNU-only flags, `get_pane_cwd` instead of /proc, chess pieces for
  human/orc symbols (U+2694/U+2620 may render as emoji), `uname -s` via run_command, dev.sh fswatch or
  GNU/BSD stat polling.
- Risks: `get_pane_cwd` on macOS, `getconf DARWIN_USER_TEMP_DIR`, BSD find/od/grep flags, perl
  Time::HiRes, ✳/spinner glyphs drawn as emoji, afplay FLAC (10.13+, from memory), two zellij servers
  with differing TMPDIR. Checklist: README "macOS checklist".

## Known issues / caveats
- Once after a reload one finish produced two identical dings (two instances acted); not reproduced.
- After a reload, tabs whose last Stop was `done ... sub` show the spinner for the grace period.
- The subagent count depends on hooks/transcripts; a missed stop sticks until stale/end/pane close.
- Sessions started before an alias/option change keep the old options; a new session picks them up.

## Status / how to resume (2026-10-20)
- Moved out of the dotfiles repo into this standalone checkout `~/code/crossatko/zellij-wc3cz-claude`
  (not yet a git repo; the owner will publish it). Added `package_dir` (replaces the old hardcoded
  `~/.config/zellij/play-phrase.sh` default), play-phrase.sh defaults to its own `sounds/wc3`, setup.sh
  now links, installs the font from `assets/`, grants both paths and sets hooks by default.
- The owner's dotfiles use it via the setup.sh link (`~/.config/zellij/plugins/claude-tabs`, git-ignored
  there), keep their own `layouts/claude.kdl`, and `install.sh` runs this setup.sh when the checkout
  exists. `~/.claude/settings.json` hooks point at this checkout's `claude-hook.sh`.
- Live sessions started before the move (profound-newt, profound-cuckoo on 2026-10-20) run the old wasm
  whose default script path `~/.config/zellij/play-phrase.sh` no longer exists: no voice lines, no
  transcript-based agent counts (log: `scan found []`) until `./dev.sh -s <session>` reloads the new wasm
  (not done by the agent) or the session is restarted.
- Open: update the external packaging script's target path; test on a Mac and other desktops;
  `git init` + first commit by the owner.
