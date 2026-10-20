# claude-tabs (zellij-wc3cz-claude): notes for agents

State and history of this package, for whoever works on it next. User-facing docs: `README.md`.
Keep entries short and factual, update in place, no secrets. The owner commits/pushes; agents do not.

## What it is
A zellij 0.45 tab-bar plugin (Rust -> wasm32-wasip1, zellij-tile 0.45.1) that replaces the stock
`compact-bar` (ported verbatim from zellij v0.45.1 `default-plugins/compact-bar`: `line.rs`, `tab.rs`,
clipboard hints; tooltips dropped) and adds Claude Code awareness: `✳` marker + Claude Code's spinner
while working, running-subagent count (`✳2`), finished-unseen badge + blink, a Warcraft III CHARACTER per
Claude tab (icon + tint from its race colour, 92 characters), a Czech WC3 voice line of that character when an agent finishes (fallback
`sounds/ding.wav`), cwd-based tab labels, clock. Linux verified; macOS written blind (see below).

## Layout of this dir (standalone; formerly part of the owner's dotfiles)
- `src/main.rs` (state, characters, sounds, hooks, timers), `src/tab.rs` (ribbon render, tint), `src/line.rs`
  (bar layout, ported), `src/clipboard_utils.rs`; `Cargo.toml`/`Cargo.lock`.
- `claude-tabs.wasm`: TRACKED prebuilt (what configs load; rebuild + commit it after every source change).
  `target/` is git-ignored.
- Packaging: `.claude-plugin/plugin.json` + `marketplace.json` (repo root = marketplace = plugin, name
  `zellij-wc3cz`), `hooks/hooks.json`, `fontconfig/` (`install.sh`, generated `60-wc3-icons.conf`),
  `setup.sh` (optional helper; no options = status only), `dev.sh` (build + hot reload).
- Shell side (POSIX sh, bash 3.2 / BSD-safe, all locate siblings via `$(dirname "$0")`):
  `claude-hook.sh` (Claude Code hook), `claude-agents.sh` (running agents from transcripts),
  `claude-ctl.sh` (ding/character/race/reshuffle via `zellij pipe`), `play-phrase.sh` (clip picker, `--list`,
  `--ding`), `os-focus.sh` (OS window focus), `claude-lib.sh` (sourced helpers, mode 644).
- `layouts/claude.kdl` (compact layout + bar, `file:~/.claude/plugins/marketplaces/zellij-wc3cz/...`), `sounds/ding.wav`, `sounds/wc3/` (library),
  `assets/wc3-icons/` (icon font + build). `.gitattributes`: flac/wav/wasm/ttf/webp/png binary (no LFS).

## Path model (2026-10-20)
- The wasm cannot learn where it was loaded from. Package dir (2026-10-20, runtime contract with the
  packaging side): option `package_dir` if non-empty; else `<state root>/pkgdir` (one line, absolute
  physical path), which `claude-hook.sh` writes on every event (atomic, only when it differs) from its OWN
  resolved dir (symlinks of the script/dir followed via readlink loop + `cd -P`; works from
  `${CLAUDE_PLUGIN_ROOT}`, a checkout, a link); else the legacy `~/.config/zellij/plugins/claude-tabs`.
  The plugin re-reads it every 1 s tick (`refresh_paths`); a change re-derives `phrase_script` (unless the
  option is set) and rescans at once, no restart. `claude-agents.sh`/`os-focus.sh` = next to
  `phrase_script` (or `agents_script`); `sounds/` via play-phrase.sh (empty `sounds_dir` = its own
  `sounds/wc3`). All scripts are run as `sh <path>` (no exec bit needed) and skipped when missing: scan
  exits 99 -> log `no .../play-phrase.sh yet`, pool = characters.tsv, retry every 5 s; ding = no-op.
  All scripts resolve their own dir the same way (play-phrase, ctl, os-focus; hook exits if it cannot).
  Caveat: with two install dirs registered (plugin + old settings.json) `pkgdir` flips between them on
  every event (harmless: both are complete packages; each flip triggers one rescan).
- No symlink and no `package_dir` are needed any more (pkgdir above). The old link
  `<zellij config dir>/plugins/claude-tabs` only for configs that still use that alias: `setup.sh --link`.
- Permission grants are keyed by `location.to_string()` = the `~`/`$VAR`-expanded path (shellexpand::full),
  symlinks NOT resolved, or the URL verbatim (zellij 0.45.1 source: wasm_bridge.rs `cache_plugin_permissions`,
  zellij-utils layout.rs `RunPluginLocation::parse`/Display). Cache `${XDG_CACHE_HOME:-~/.cache}/zellij/
  permissions.kdl`, macOS `~/Library/Caches/org.Zellij-Contributors.Zellij/permissions.kdl`.
  `setup.sh --permissions` (opt-in) appends grants for the checkout's physical path, the path setup.sh
  was called by, and the legacy link if it points here.
- Multiple Claude config dirs need nothing at runtime: the hook takes `transcript_path` from the payload,
  the plugin and `claude-agents.sh` only use the dir the hook recorded; nothing else references
  `~/.claude` (grep of src/ and scripts).

## Install / packaging (2026-10-20, owner policy: change NO user config by default)
- Route: Claude Code plugin for the hooks + a zellij layout for the bar; everything else optional and
  explicit. `setup.sh` without options = status/dry run (verified: fake HOME snapshot byte-identical).
  Opt-in options: `--font`/`--font-uninstall`, `--permissions`, `--build`, `--migrate-to-plugin`,
  legacy `--link`, `--legacy-settings-hooks` (old `--hooks`; `--no-hooks` accepted, no-op).
  NOTE the owner's dotfiles `install.sh` calls `sh setup.sh` with no options, which now changes nothing
  (it used to link, install the font, pre-grant, write hooks). Not changed by the agent.
- Claude Code plugin (docs checked 2026-10-20: code.claude.com/docs/en/plugins-reference,
  /plugin-marketplaces, /plugins/loading, /hooks): name must not start with `claude-` (reserved, validate
  error), so plugin and marketplace are `zellij-wc3cz`; marketplace entry `source: "./"` (repo root).
  `hooks/hooks.json` = the 11 events setup.sh wrote (SessionStart start, UserPromptSubmit working,
  PreToolUse tool, PermissionRequest ask, Notification notify, PostToolUse/PostToolUseFailure clear, Stop
  done, SubagentStart substart, SubagentStop substop, SessionEnd end), no matchers, no timeouts (default
  600 s), command `"${CLAUDE_PLUGIN_ROOT}/claude-hook.sh" <arg>` (quoted per docs). No `version` in
  plugin.json on purpose: version = git SHA, so users follow commits (validate warns, passes).
  Docs: a plugin's hook and an identical settings hook both run ("stays separate"); dedupe only across
  settings files. Plugin hooks merge with user/project hooks.
- Where things land (verified with `claude` 2.1.303, scratch HOME + CLAUDE_CONFIG_DIR): plugins root =
  `$CLAUDE_CONFIG_DIR/plugins` (default `~/.claude/plugins`). Git/GitHub marketplace = full clone at
  `plugins/marketplaces/zellij-wc3cz/` (stable path, contains claude-tabs.wasm -> the layout's location)
  + a COPY per version at `plugins/cache/zellij-wc3cz/zellij-wc3cz/<12-char sha>/` (exec bits kept;
  `${CLAUDE_PLUGIN_ROOT}` = this dir, changes on update; old dirs removed 14 days later). Local-path
  marketplace = loaded in place (CLAUDE_PLUGIN_ROOT = the checkout, no copy). Disk: clone + each cached
  version carry the ~138 MB sound library. The marketplace clone updates on `claude plugin marketplace
  update` / plugin update (auto-update is OFF by default for third-party marketplaces).
- zellij plugin loading (0.45.1 source): `file:/abs`, `file:~/...` (tilde + env expansion), relative
  `file:x` = joined with the session/layout cwd (NOT the layout file's dir); `https://` URLs are downloaded
  ONCE to the cache dir as a hash of the URL and never refreshed while that file exists (unsuitable for
  updates). `zellij --layout X` inside a session or with `-s` ADDS tabs to that session; `zellij -n X`
  always starts a new session. Plugin options can be given in the layout's plugin block.
- PERMISSION PROMPT PROBLEM (verified in isolated session pkgtestb, current wasm): the prompt is drawn in
  the 1-row bar ("This plugin asks permission to: ... Allow? (y/n)"), but the plugin calls
  `set_selectable(false)` before `request_permission`, so the pane cannot be focused (`move-focus` stays on
  the terminal; `list-panes -a` shows is_selectable false) and `y` goes to the shell: the default
  "press y" path does not work. Needs the runtime side (src) to stay selectable until
  PermissionRequestResult. With `setup.sh --permissions` (grant for the expanded marketplace path) the
  plugin loaded without a prompt (verified, same session setup).
- `fontconfig/install.sh` (Linux/BSD; macOS/others print "not supported, initials badges", exit 0):
  copies the font to `${XDG_DATA_HOME:-~/.local/share}/fonts/wc3-icons.ttf`, writes
  `${XDG_CONFIG_HOME:-~/.config}/fontconfig/conf.d/60-wc3-icons.conf` GENERATED from the installed font's
  charset (`fc-query %{charset}`; repo copy `fontconfig/60-wc3-icons.conf` = `install.sh --print-conf`,
  regenerate after font changes), fc-cache, prints fc-match proof + per-terminal lines (edits no terminal
  config). Refuses to overwrite a conf without its marker; `--uninstall` removes only marked/WC3 files;
  `--check`. Rules: (1) `emoji` alias prefer WC3 Icons (kitty 0.49.2 looks up emoji-presentation chars
  as family "emoji"+color via FcFontSort WITHOUT charset, then picks the first candidate covering the char:
  read in kitty/fontconfig.c v0.49.2); (2) weak append of WC3 Icons to every pattern (FcFontSort-list
  terminals; weak = never beats a requested family: `fc-match monospace` unchanged, strong append DID
  hijack it); (3) per-codepoint `charset contains` -> prepend_first strong (FcFontMatch with charset).
  Verified in a sandbox (HOME/XDG_* in scratchpad, system fonts incl. Noto/Apple Color Emoji, Font
  Awesome): all 1260 codepoints of U+1F7E0-1F7EB, U+1FA70-1FAFF, U+1F300-1F64F, U+1F900-1F9FF compared
  before/after for `fc-match emoji:color=true:charset=X`, `monospace:charset=X` and kitty's own matcher
  (`kitty +runpy` fast_data_types.fc_match_fallback, i.e. kitty's real code path, headless): every font
  codepoint -> WC3 Icons; 0 changes for codepoints another font really covers (U+1F600, U+1FAE0, U+1FAD7,
  U+1F7EB stay Noto); 338 changes only where neither old nor new font has the glyph (tofu either way).
  Known side effect: `fc-match emoji` without a charset now answers WC3 Icons (apps that pick ONE emoji
  font without charset would be wrong); system-wide those ~100 emoji show WC3 portraits.
  NOT verified: rendering in a real kitty/alacritty/foot window (kitty only via its matcher), ghostty
  (icon agent: ignores fontconfig, needs font-codepoint-map), the owner's real fontconfig (not touched).
  The `font` flag (hook) only says the font is installed, not that the terminal draws it.
- Shipped layout `layouts/claude.kdl`: compact layout, `file:~/.claude/plugins/marketplaces/zellij-wc3cz/
  claude-tabs.wasm`; loaded in isolated session pkgtestb with fake HOME (tilde expanded to it, log
  `Loaded plugin '.../marketplaces/zellij-wc3cz/claude-tabs.wasm'`). Live owner sessions untouched.
- Verified setup.sh on fake HOMEs: dry run writes nothing; `--legacy-settings-hooks` adds 11 events,
  repoints `/old/place/claude-hook.sh tool` and a quoted path with a space, rerun byte-identical, invalid
  JSON = rc 1 untouched, missing dir skipped; with the plugin installed (scratch config) status reports
  DOUBLE REGISTRATION; `--migrate-to-plugin` removes exactly `claude-hook.sh <our arg>` handlers (an
  unrelated `/other/tool/claude-hook.sh --mine` kept, other hooks/keys kept, emptied groups/events and an
  emptied `hooks` dropped), backup = pre-migration file, rerun identical, empty file left alone;
  `--permissions`/`--link` idempotent. Real hook firing through the plugin: `claude -p --plugin-dir <copy>
  --setting-sources project --no-session-persistence` (fake zellij env, session name pkgtestb) ran start,
  working, done, end with CLAUDE_PLUGIN_ROOT = the plugin dir. Marketplace add+install verified from a
  local dir and from a git URL (local smart-HTTP server); NOT verified: `crossatko/zellij-wc3cz-claude`
  from GitHub (not published), the interactive `/plugin` UI, the one-step
  `/plugin install zellij-wc3cz --marketplace crossatko/zellij-wc3cz-claude` (suggested by validate).
- How to resume: `claude plugin validate .` (expect only the version warning); `sh setup.sh` (status);
  sandbox tests need HOME/XDG_*/CLAUDE_CONFIG_DIR in a scratch dir (never the real ones). After a font
  change: `sh fontconfig/install.sh --print-conf > fontconfig/60-wc3-icons.conf`.

## Build / dev / test
- Toolchain: Arch `sudo pacman -S rust rust-wasm`; macOS `brew install rustup && rustup-init -y &&
  rustup target add wasm32-wasip1`. Build: `cargo build --release --target wasm32-wasip1`, then copy
  `target/wasm32-wasip1/release/claude-tabs.wasm` over `claude-tabs.wasm` (`dev.sh` and `setup.sh --build` do this).
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
  `<pane>.ev` (activity), `<pane>.sess` (agent transcript dir), `characters/<tab_id>` (character name; the old `races/` dir is unused), `names/<tab_id>`;
  `last-<character>` (last clip, name sanitised).
- Attention (2026-10-20): hooks `PermissionRequest ask`, `Notification notify` (no matcher; script keeps
  permission_prompt/elicitation_dialog/elicitation_url_dialog, ignores idle_prompt etc. without touching
  `.ev`), `PreToolUse tool` with tool_name AskUserQuestion/ExitPlanMode, all write the OVERLAY file
  `<pane>.attn` = `<ns>[ away] ag=<agent_id|->` (not a state token: it leaves the working/done state and
  the subagent/grace logic untouched). Existing overlay = no rewrite, so the three signals of one request
  ding once. Plugin: `St::Attn` wins in `eff_state` (beats working_hold), rank highest, `?` inverted
  underlined badge + blink (`tab_blink`/`tab_away` handle it); leader dings once per ns (`attn_reacted`,
  `attention_repeat_s`), no auto-switch; on the active tab it is deleted at once unless `away` (then kept
  until os-focus says focused). Cleared by: `clear` (PostToolUse/Failure), a tool event, SubagentStop of
  the asker, UserPromptSubmit, Stop, start, end; an event with an `agent_id` clears only an overlay with
  the same id or `-`. A Stop while the overlay exists writes `done ... quiet` (no finish ding). Docs say
  denied permissions fire no hook (next tool event / Stop clears) and PermissionDenied is auto-mode only.
  Overlays older than 12 h are ignored. Verified in isolated session pktest with simulated payloads (not
  real Claude Code payloads: Notification/PermissionRequest agent_id for subagents unconfirmed).
- `<state root>/font` (= `1`/`0`): the hook runs `fc-list 'WC3 Icons' family` in the BACKGROUND when the file
  is missing or older than 10 min (`find -mmin`; touched first so parallel hooks do not re-check); Darwin and
  no fc-list = 0. Note it depends on the hook's $HOME (fontconfig user dir). Plugin `race_font auto` = this
  flag (was: Linux on, macOS off); `on`/`off` force.
- Duplicate hooks (plugin hooks + settings.json from an older setup.sh; Claude Code dedupes only identical
  command strings): every event except tool/clear does `mkdir <pane>.once.<cksum of event+JSON>`; the loser
  exits. Any other event of the pane (tool/clear included, cheap glob) removes old markers, so a later
  genuine identical Stop is handled (relies on Claude Code finishing one event's hooks before the next).
  Temp files are `.$$`-suffixed (the shared `<pane>.tmp` raced: mv errors seen), the attention overlay is
  created with `ln` (exactly one of concurrent raises wins). `end` and the plugin's `cleanup_stale` remove
  markers (and `.sess`). Verified: unit test with stub zellij/fc-list (10x concurrent Stop pairs with a
  pending question: one poke, `quiet` kept; identical Stops separated by tool/substop: both handled), and
  in an isolated session: concurrent Stops from two install dirs, 3 rounds -> 3 clips.
- Hooks: SessionStart start, UserPromptSubmit working, PreToolUse tool, Stop done, SubagentStart
  substart, SubagentStop substop, SessionEnd end. Silent no-op outside zellij. Poke =
  backgrounded `zellij pipe --name claude_poke` (logs a harmless `CliPipe did not complete within 1s`);
  state is polled anyway (1 s). Without hook state a pane counts as Claude by command/title
  (`title_fallback`).
- One instance per tab; instances in INACTIVE tabs get no Tab/PaneUpdate/Visible events (stale data;
  after a reload also no ModeUpdate, so the session name is learned from `get_session_list()`). Only the
  LEADER (lowest live claude-tabs plugin pane id in a fresh `get_session_list()` snapshot) dings,
  auto-switches, acknowledges Done -> Idle, assigns characters and deletes state of closed panes. `perms`
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
- Characters (replaced per-tab races, 2026-10-20): `assets/wc3-icons/characters.tsv` (race, character,
  codepoint U+1FA70..U+1FAD6, clips, icon_file) is embedded with `include_str!` (REBUILD the wasm after the icon
  agent changes it; a row with empty icon_file uses the race glyph). A character is the filename part before
  the first " - " in `sounds/wc3/<race>/`; "Trollí čarodějnice - lékařka" contains " - " itself, so the scan
  (`play-phrase.sh --list` = `race<TAB>prefix` lines) is resolved against the tsv (`resolve_scan`: name ==
  prefix or starts with prefix + " - "); unknown names stay as listed (race icon, hashed accent).
  Pool = option `characters` (comma separated, names have spaces) else every scanned character with clips
  (all tsv rows before the first scan / with no library; assignment waits for the first scan), narrowed by
  option `races`. Least-used first, random among ties; stored per stable `tab_id` (`create_new`, first
  writer wins). `claude-ctl.sh character <name>` / `race <race>` (least-used character of the race) /
  `ding [character]` / `reshuffle`; pipe names `claude_character`, `claude_race`, `claude_ding`.
  play-phrase.sh matches files `"<Character> - "*` (awk index on ENVIRON, no globbing) in all race folders;
  character without clips = any clip; nothing = exit 3 -> ding.
  Without the colour font (`race_font` off/auto with font flag 0) a tab shows a 2-cell INITIALS badge instead of
  the glyph (`badge_candidates`/`badge_table` in main.rs, `badge_colors` in tab.rs): first letters of words 1+2
  (single word: letters 1+2), words reduced to width-1 letters (so `-`, `(`, `'` vanish), uppercase
  (multi-char uppercase keeps the letter). Collisions resolved over the WHOLE tsv in tsv order (not the pool:
  same badge in every session): first holder keeps it, later ones take word-1 initial + the next letter
  (word 2, rest of word 1, others) that nobody has as first choice. All 92 unique, 2 cells, BMP
  (`cargo test --target x86_64-unknown-linux-gnu -- --nocapture` prints the table), e.g. AM Arthas
  Menethill, AN Arthas (Nemrtví), SW Sea witch / SI Sylvanas / SN Sylvanas (Nemrtví), TČ, ŠA, PĚ. Unknown
  library names are appended after the tsv. Chip = accent at (dim+0.35) on inactive tabs, 0.55 on the active
  one, black/white text; flush left like the icon, so tab widths equal the font mode (checked: same clock
  column). Rendered from an isolated session (pyte replay of the client output + Pillow image of the bar row).
  Options `race_icons/race_font/race_tab_colors/
  race_dim/race_all_tabs` keep their names. Tint = `character_accent` in `src/main.rs`: RACES colour in HSL,
  hue +-12 deg, lightness +-8 % (+-12 % grey neutral), saturation +-8 %, spread per index in the tsv
  with golden-ratio sequences; `tint_colors` still guarantees 4.5:1 text.
- Icon codepoints (2026-10-20, FIXED): private-use codepoints (first try U+F0100..) are drawn by kitty as
  1-cell MONOCHROME masks (grey/tinted blobs); also U+E0100, CJK U+20000/U+2A700/U+30000 and unassigned
  U+1F7EC/U+1FA00 (screenshots in an isolated kitty with a private fontconfig + test family). Only
  emoji-presentation codepoints get the colour bitmap, so characters now use U+1FA70..U+1FAD6 (first 92
  width-2 codepoints of U+1FA70.. per kitty's width, all 2 for `unicode-width` 0.2.2), 2 cells wide like the race
  glyphs: tab.rs pads one space after any icon (`is_icon_font_char`), no extra padding. Side effect: kitty
  shows WC3 icons for those emoji (symbol_map U+1FA70-U+1FAFF). Verified by screenshot: colour, whole faces,
  aligned with text, on tinted backgrounds. Crop: build.py keeps the whole icon (horizontal squash to ASPECT)
  and crops the 4 px frame only when `has_frame()` sees it (all 99 do).
  Not verified in the real zellij tab bar by screenshot (isolated kitty only printed glyphs/SGR).
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
  nightelf, naga, demon, neutral), 2 cells wide. Installed on Linux only on request by
  `fontconfig/install.sh` (= `setup.sh --font`) to `~/.local/share/fonts` + fontconfig rules (see
  "Install / packaging"); kitty then needs no `symbol_map` (verified via kitty's matcher only). `race_font "auto"` = the
  hook's font flag (installed per fc-list; macOS always 0: CoreText has no CBDT, an sbix variant would be needed).
- Rebuild: `cd assets/wc3-icons && python3 build.py` (defaults 2026-10-20: FAMILY "WC3 Icons", PPEM 96, S 144, W 112
  (ASPECT 0.7778), BY 116; icon centre-cropped to ASPECT, never squashed, 22% side crop). Why these numbers: kitty
  scales a strike by font_px/PPEM (font_size 18 = 24 px -> x0.25) and shrinks it by whole px once it exceeds the
  2-cell box, so the bitmap must scale to EXACTLY 28x36 at the owner's 14x36 cells. The old 145/PPEM 97 gave 35.88 px
  = a half-covered top row (thin background line above every icon; measured 0.41-0.5 alpha, isolated kitty with
  the owner's real conf, solid test bitmaps, grim crop of the test window + Pillow). S 146-150 at PPEM 97 is worse
  (shrunk to ~35 px). BY moves the icon 0.25 px/unit (115 = top row partial, 117 = bottom row partial). Verified
  14x36: 28.0x36.0 px, 0 gaps, two stacked lines seamless. Retune = S/PPEM so S*font_px/PPEM == cell height and
  W*font_px/PPEM <= 2*cell width (CBDT byte limits height/width <= 255, BY <= 127). Test builds need another family
  name AND a private FONTCONFIG_FILE + XDG_DATA_HOME (the user fonts dir is scanned otherwise).
- Other terminals (measured, README has the table): kitty OK (symbol_map; other cell sizes undistorted but smaller
  than the box, only 14x36 pixel exact). alacritty 0.17: no symbol_map, uses the icon font only if a fontconfig rule
  appends it to the terminal font's family list (stock fontconfig shows other fonts' glyphs/emoji), draws 27x35 at
  size 18 unfitted: needs `font.offset.y = 6`. ghostty 1.3.1: ignores fontconfig for emoji codepoints, needs
  `font-codepoint-map = U+1F7E0-U+1F7E6=WC3 Icons` (+ U+1FA70-U+1FAD6), scales to the natural cell (22x29 at 15x29,
  gaps with adjusted line height). wezterm/foot/konsole/gnome-terminal/xterm/st/etc. are not installed: untested.
  Mitigations suggested, not implemented (fontconfig snippet from setup.sh, document codepoint-map, `race_font off`).
  Test harness lived in the agent scratchpad (floating kitty/alacritty/ghostty on the secondary monitor via
  `hyprctl eval "hl.exec_cmd(cmd,{float=true,monitor=...,no_initial_focus=true,...})"`; `hl.window_rule` via eval did
  NOT apply on this Hyprland 0.56 (window tiled and stole focus once); frame-coloured test bitmaps for measuring).
  Old tuning workspace (outside the repo): `~/.local/share/wc3-icons/`.

## macOS (written without a Mac; unverified)
- Done for it: claude-lib.sh (Homebrew PATH, state-root candidates, `run_timeout` timeout/gtimeout/perl,
  `now_ns` without `%N`), no GNU-only flags, `get_pane_cwd` instead of /proc, chess pieces for
  human/orc symbols (U+2694/U+2620 may render as emoji), `uname -s` via run_command, dev.sh fswatch or
  GNU/BSD stat polling.
- Risks: `get_pane_cwd` on macOS, `getconf DARWIN_USER_TEMP_DIR`, BSD find/od/grep flags, perl
  Time::HiRes, ✳/spinner glyphs drawn as emoji, afplay FLAC (10.13+, from memory), two zellij servers
  with differing TMPDIR. Checklist: README "macOS checklist".

## Known issues / caveats
- FIXED (2026-10-20) delayed ding (played only after switching tabs): `task_running` (transcript agent scan
  from `claude-agents.sh`) is per instance and only the instance of the FOCUSED tab polled it, so the
  leader in an inactive tab kept a stale non-empty list, `read_files` counted it as running subagents and
  `eff_state` kept the finished tab Working until the leader's own tab was focused again. Fix: entries
  carry a timestamp (`task_at`) and are ignored after 10 s; the leader also polls (`ask_agents`) from its
  1 s tick. Reproduced and verified in an isolated session (3 tabs, stub player, stale agent in pane of
  tab C, leader tab A inactive: old wasm dinged only after go-to-tab 1; new wasm dinged ~3 s after Stop,
  once). Plain finishes in inactive tabs were already instant (also verified). Not verified on the live
  sessions (they need `./dev.sh -s <session>`).
- Once after a reload one finish produced two identical dings (two instances acted); not reproduced. (A
  hook registered twice could also double-ding before 2026-10-20; now deduplicated, see How it works.)
- The font flag says "installed", not "this terminal draws it" (alacritty/ghostty need config): such users
  set `race_font "false"`.
- After a reload, tabs whose last Stop was `done ... sub` show the spinner for the grace period.
- The subagent count depends on hooks/transcripts; a missed stop sticks until stale/end/pane close.
- Sessions started before an alias/option change keep the old options; a new session picks them up.

## Status / how to resume (2026-10-20)
- Moved out of the dotfiles repo into this standalone checkout `~/code/crossatko/zellij-wc3cz-claude`
  (not yet a git repo; the owner will publish it). Added `package_dir` (replaces the old hardcoded
  `~/.config/zellij/play-phrase.sh` default), play-phrase.sh defaults to its own `sounds/wc3`, setup.sh
  (2026-10-20 later: packaging reworked, see "Install / packaging"; setup.sh no longer changes anything by
  default).
- The owner's dotfiles use it via the old link (`~/.config/zellij/plugins/claude-tabs`, git-ignored
  there; it still exists, setup.sh leaves it), keep their own `layouts/claude.kdl`, and `install.sh` runs
  `sh setup.sh` without options (now a status-only dry run: to keep the old effect it would need
  `--link --font --permissions`, or switch to the plugin route). `~/.claude/settings.json` hooks point at
  this checkout's `claude-hook.sh` (legacy route; installing the plugin too would double every hook:
  run `setup.sh --migrate-to-plugin` then).
- Live sessions started before the move (profound-newt, profound-cuckoo on 2026-10-20) run the old wasm
  whose default script path `~/.config/zellij/play-phrase.sh` no longer exists: no voice lines, no
  transcript-based agent counts (log: `scan found []`) until `./dev.sh -s <session>` reloads the new wasm
  (not done by the agent) or the session is restarted.
- Open: update the external packaging script's target path; test on a Mac and other desktops;
  `git init` + first commit by the owner.

## TODO: macOS icon font (needs a Mac to test)
- The WC3 icon font is CBDT/CBLC (Google colour bitmaps), built by `assets/wc3-icons/build.py` from the 7 webp icons. Believed (NOT tested) that macOS CoreText, which kitty and Ghostty use there, does not draw CBDT; it draws Apple's `sbix`. The user thinks kitty/ghostty on macOS may support it, so test first: install the current font, set `race_font on`, and look.
- If the icons don't render: add an `sbix` build (same codepoints U+1F7E0..U+1F7E6, 2 cells wide) to build.py, have `setup.sh` install it on Darwin, and keep `race_font` auto-off until verified. Tune the bitmap size per terminal (cell-height shrink behaviour seen in kitty on Linux may differ).
- Until then macOS uses the initials badges (`race_font` auto = off on Darwin: the hook writes font 0).
