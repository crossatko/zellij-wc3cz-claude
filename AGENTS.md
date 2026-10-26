# claude-tabs (zellij-wc3cz-claude): notes for agents

Current state of this package plus short caveats, for whoever works on it next. User-facing docs: `README.md` (short) and `docs/REFERENCE.md` (full).
Keep entries short and factual, update in place, no histories, no secrets, no personal setup details
(hostnames, home paths, session names, monitors). Commit small logical commits and push; no AI attribution.

## What it is
A zellij 0.45 tab-bar plugin (Rust -> wasm32-wasip1, zellij-tile 0.45.1) that replaces the stock
`compact-bar` (ported from zellij v0.45.1 `default-plugins/compact-bar`: `line.rs`, `tab.rs`, clipboard
hints; tooltips dropped) and adds Claude Code awareness: `✻` marker + Claude Code's spinner while working,
running-subagent count (`✻2`), finished-unseen badge + blink, attention alerts (`?` badge), one Warcraft III
CHARACTER per Claude tab (icon + tint + voice, 91 characters with clips), a Czech WC3 voice line of that
character when an agent finishes or asks (fallback `sounds/ding.wav`), cwd-based tab labels, clock, optional
character name next to the clock. Linux verified; macOS install and icons verified on one Mac, macOS runtime
(hooks, afplay, os-focus) not yet.

## Layout of the repo
- `src/main.rs` (state, characters, sounds, hooks, timers), `src/tab.rs` (ribbon render, tint), `src/line.rs`
  (bar layout, ported), `src/clipboard_utils.rs`; `Cargo.toml`/`Cargo.lock`.
- `claude-tabs.wasm`: TRACKED prebuilt (what layouts load; rebuild + commit it after every source change).
  `target/` is git-ignored.
- Packaging: `.claude-plugin/plugin.json` + `marketplace.json` (repo root = marketplace = plugin, name
  `zellij-wc3cz`), `hooks/hooks.json`, `fontconfig/` (`install.sh`, `install-mac.sh`, generated
  `60-wc3-icons.conf`), `setup.sh` (optional helper), `dev.sh` (build + hot reload), `layouts/claude.kdl`
  (compact layout + bar).
- Shell side (POSIX sh, bash 3.2 / BSD-safe, all locate siblings via their own resolved dir):
  `claude-hook.sh` (Claude Code hook), `claude-agents.sh` (running agents from transcripts),
  `claude-ctl.sh` (ding/character/race/reshuffle via `zellij pipe`), `play-phrase.sh` (clip picker,
  `--list`, `--ding`), `os-focus.sh` (OS window focus), `claude-lib.sh` (sourced helpers, mode 644).
- `sounds/ding.wav`, `sounds/wc3/` (library), `assets/wc3-icons/` (icon fonts + build, `characters.tsv`).
  `.gitattributes`: flac/wav/wasm/ttf/webp/png binary (no LFS).

## Path model
- The wasm cannot learn where it was loaded from. Package dir = option `package_dir` if non-empty; else
  `<state root>/pkgdir` (one line, absolute physical path), which `claude-hook.sh` writes on every event
  (atomic, only when it differs) from its OWN resolved dir (symlinks followed; works from
  `${CLAUDE_PLUGIN_ROOT}`, a checkout, a link); else the legacy `~/.config/zellij/plugins/claude-tabs`.
  The plugin re-reads it every 1 s tick (`refresh_paths`); a change re-derives `phrase_script` (unless that
  option is set) and rescans at once. No symlink and no `package_dir` are needed. `claude-agents.sh` and
  `os-focus.sh` live next to `phrase_script` (or `agents_script`); `sounds/` via play-phrase.sh (empty
  `sounds_dir` = its own `sounds/wc3`). Scripts run as `sh <path>` (no exec bit needed) and are skipped when
  missing (scan exits 99 -> log `no .../play-phrase.sh yet`, pool = characters.tsv, retry every 5 s).
  `pkgdir` is ONE global file (last hook writer wins): with two install dirs registered it flips on every
  event, so the plugin keeps its current package dir once a library scan found `play-phrase.sh` there
  (`choose_pkg`, `pkg_ok`) and switches only when that scan fails (exit 99) or `package_dir` says so.
  `phrase_script ""` disables clips only; agent counts and os-focus look for their scripts next to the
  package dir.
- Permission grants are keyed by `location.to_string()` = the `~`/`$VAR`-expanded path (symlinks NOT
  resolved) or the URL verbatim (zellij 0.45.1 `wasm_bridge.rs`, layout.rs). Cache
  `${XDG_CACHE_HOME:-~/.cache}/zellij/permissions.kdl`, macOS
  `~/Library/Caches/org.Zellij-Contributors.Zellij/permissions.kdl`. `setup.sh --permissions` appends grants
  atomically for the checkout path, the path setup.sh was called by and the legacy link if it points here.
- Multiple Claude config dirs need nothing at runtime: the hook takes `transcript_path` from the payload;
  nothing else references `~/.claude`.

## Install / packaging (policy: change NO user config by default)
- `PROMPT.md` (repo root) is the agent install guide (README's recommended route: the user pastes a prompt, their
  Claude Code agent clones the repo and follows it). Keep it in sync whenever install steps, flags, paths,
  terminal config lines or options change (also README Quick start and `docs/REFERENCE.md`).
- Route: Claude Code plugin for the hooks + a zellij layout for the bar; everything else optional.
  `setup.sh` without options = status/dry run, writes nothing. Opt-in: `--font`/`--font-uninstall`,
  `--permissions`, `--build`, `--migrate-to-plugin`, `--link` (legacy alias path), `--legacy-settings-hooks`
  (old `--hooks`, deprecated; `--no-hooks` accepted, no-op); targets `--settings FILE` (repeatable),
  `--all-claude-dirs`, else `$CLAUDE_SETTINGS`, `$CLAUDE_CONFIG_DIR/settings.json`, `~/.claude/settings.json`.
  `--help` prints only the header. Status reports DOUBLE REGISTRATION when plugin and settings hooks coexist;
  `--migrate-to-plugin` removes exactly our `claude-hook.sh <arg>` handlers (other hooks/keys kept,
  backup `<file>.bak-claude-tabs`, idempotent). Old hooks in Claude settings double every hook once the
  plugin is installed: run `--migrate-to-plugin`.
- The `claude` CLI itself (not our scripts) adds `extraKnownMarketplaces`/`enabledPlugins` to settings.json.
- Plugin: the name must not start with `claude-` (reserved), so plugin and marketplace are `zellij-wc3cz`,
  marketplace entry `source: "./"`. No `version` in plugin.json on purpose (version = git SHA; validate
  warns, passes). `hooks/hooks.json` has the 11 events, no matchers, no timeouts, command
  `"${CLAUDE_PLUGIN_ROOT}/claude-hook.sh" <arg>`:
  SessionStart `start`, UserPromptSubmit `working`, PreToolUse `tool`, PermissionRequest `ask`,
  Notification `notify`, PostToolUse and PostToolUseFailure `clear`, Stop `done`, SubagentStart
  `substart`, SubagentStop `substop`, SessionEnd `end`. A plugin hook and an identical settings hook BOTH run.
- Where things land (Claude Code 2.1.303): plugins root `$CLAUDE_CONFIG_DIR/plugins` (default
  `~/.claude/plugins`), so each config dir needs its own marketplace add/install; one layout serves all (hooks
  report `pkgdir`). Git marketplace = full clone at `plugins/marketplaces/zellij-wc3cz/` (stable path with
  the wasm, what the layout loads) + a COPY per version at
  `plugins/cache/zellij-wc3cz/zellij-wc3cz/<12-char sha>/` (`${CLAUDE_PLUGIN_ROOT}`, changes on update). A
  local-path marketplace loads in place. The cache copy is made from git by Claude Code, so it only differs
  from HEAD if it is an OLDER version (see macOS caveat) or hand-edited. Disk: clone ~270 MB + ~140 MB per
  cached version (sound library). Add + install verified from a local dir, a local git URL and the real
  GitHub repo (HTTPS or SSH, follows the user's git config); NOT the interactive `/plugin` UI.
- zellij plugin loading (0.45.1 source): `file:/abs`, `file:~/...` (tilde + env expansion), relative
  `file:x` = joined with the session/layout cwd; `https://` URLs are downloaded ONCE and never refreshed.
  `zellij --layout X` inside a session or with `-s` ADDS tabs; `zellij -n X` starts a new session. A plain
  `zellij` with `default_layout "compact"` never loads the plugin. Plugin options go in the layout's plugin
  block and are fixed at session start (a running session keeps old options; a reload picks up only a new
  wasm). Shipped layout loads `file:~/.claude/plugins/marketplaces/zellij-wc3cz/claude-tabs.wasm`
  (`${CLAUDE_CONFIG_DIR:-~/.claude}` for other config dirs).
- `fontconfig/install.sh` (Linux/BSD; Darwin execs `install-mac.sh`; other OSes print "not supported,
  initials badges", exit 0): creates `conf.d/` if needed, copies `wc3-icons.ttf` and `wc3-icons-wide.ttf` to
  `${XDG_DATA_HOME:-~/.local/share}/fonts/` (never overwrites a foreign file of that name), writes
  `${XDG_CONFIG_HOME:-~/.config}/fontconfig/conf.d/60-wc3-icons.conf` GENERATED from the font's charset (repo
  copy = `install.sh --print-conf`; regenerate after font changes:
  `sh fontconfig/install.sh --print-conf > fontconfig/60-wc3-icons.conf`), `fc-cache -f` on the font dir,
  removes the `<state root>/font` flag file, prints `--check` and per-terminal config lines. Refuses to
  overwrite a conf without its marker; `--uninstall` (also removes a `WC3 Icons Custom` build at
  `fonts/wc3-icons-custom.ttf`), `--check`. Rules: (1) per character codepoint `charset contains` ->
  prepend_first `WC3 Icons` (race circles U+1F7E0.. untouched); (2) scan rules giving the families
  `WC3 Icons`, `WC3 Icons Wide`, `WC3 Icons Custom` lang `und-zsye` (fcft/foot takes emoji-presentation
  codepoints only from fallbacks with that lang; needs fc-cache -f). `fc-match emoji`, `:lang=und-zsye`,
  `emoji:charset=1f600` still answer Noto Color Emoji. Icons need a full terminal restart.
- Sandbox rule: tests of setup.sh / plugin install need HOME/XDG_*/CLAUDE_CONFIG_DIR in a scratch dir.
  `claude plugin validate .` should only warn about the missing version.

## Build / dev / test
- Toolchain: Arch `sudo pacman -S rust rust-wasm`; macOS `brew install rustup && rustup-init -y &&
  rustup target add wasm32-wasip1`. Build: `cargo build --release --target wasm32-wasip1`, copy
  `target/wasm32-wasip1/release/claude-tabs.wasm` over `claude-tabs.wasm` (`dev.sh`, `setup.sh --build`).
  Unit tests: `cargo test --target x86_64-unknown-linux-gnu`; `sh tests/subagents.sh` (hook/agents scripts,
  private TMPDIR, stub zellij), `sh tests/diversity.sh`. `cargo test` runs on the host, so std calls that
  panic only under WASI (process ids, threads, Command) are caught only by the
  `no_wasi_unsupported_std_calls` source guard (no process::id/Command/thread::spawn/env::current_exe|dir
  outside tests); extend its list when adding std calls. Temp file names in the plugin use the plugin id.
- Hot reload: `./dev.sh [-s SESSION | --all] [--watch] [-p URL | -a [NAME]]` = build, copy, then
  `zellij --session S action start-or-reload-plugin X` (reloads every per-tab instance of the plugin loaded
  as exactly X; needs a connected client, else `No connected clients, cannot reload plugin`: attach a pty when
  headless). X that matches nothing LAUNCHES a new plugin pane in the focused tab, so dev.sh first checks
  `list-panes -a` (columns split by two spaces; column 7 = the location string the pane was loaded with) and
  only reloads with a target present in the session: autodetect = alias `claude-tabs`, else
  `file:<checkout>/claude-tabs.wasm`, else REFUSE with rc 1 (also for `-p`/`-a` values that are not present).
  Layouts loading another path/symlink/marketplace clone need `-p` with that exact string. `--watch`:
  inotifywait, fswatch, entr, else 1 s polling (macOS: fswatch or stat polling). A dead instance (plugin
  panic) only recovers with a reload.
- Test in an ISOLATED session with its own `ZELLIJ_CONFIG_DIR`/`XDG_CACHE_HOME`, a test wasm copy and a stub
  player; drive it with simulated hook calls (`ZELLIJ=0 ZELLIJ_SESSION_NAME=<test> ZELLIJ_PANE_ID=N sh
  claude-hook.sh done </dev/null`). Never kill or reshuffle someone's live sessions. No screenshots of the
  whole screen: text/SGR dumps, or crop only the test window; floating test windows must not steal focus.
  On macOS `screencapture -l <id>` (window found by pid; needs Screen Recording for the host app).
- Logs: `grep claude-tabs /tmp/zellij-$(id -u)/zellij-log/zellij.log` (`scan found [...]`,
  `tab N -> character`, `ding_command failed`). Hook debug: `touch <state root>/debug` logs every event + JSON
  to `<state root>/debug.log`. Ding log: `<state root>/ding.log`.
- Do not `zellij pipe` this plugin bare: it blocks until every per-tab instance answers; `claude-ctl.sh`
  caps it with a timeout and sends the payload on stdin.
- Sanity: all scripts pass `sh -n` and `bash --posix -n`. Resume: `sh setup.sh` (status), `./dev.sh -s
  <isolated session>`, `claude plugin validate .`.

## How it works
- State root = the dir the plugin sees as `/tmp` = `<temp_dir()>/zellij-<uid>`; `claude_state_root` picks
  `$TMPDIR`, `/tmp` or `getconf DARWIN_USER_TEMP_DIR`, the first containing `zellij-<uid>/zellij-log`.
  Files under `zellij-claude/<session>/`: `<pane>` (`idle|working|done <ns> [sub] [away] [quiet]`),
  `<pane>.sub/<agent_id>` markers, `<pane>.ev` (activity), `<pane>.sess` (agent transcript dir),
  `<pane>.attn` / `<pane>.attn-seen` (attention, below), `<pane>.agents` (shared agent scan, below),
  `characters/<tab_id>`, `names/<tab_id>`, `last-<character>` (last clip, name sanitised); at the root also
  `pkgdir`, `font`, `play.lock`, `playing`, `queued`, `ding.log`, `library-<session>`, `silence-<ms>.wav`,
  and `/tmp/zellij-claude/recent-characters` (cross-session, 16 lines).
- Hooks are a silent no-op outside zellij. Poke = backgrounded `zellij pipe --name claude_poke` (logs a
  harmless `CliPipe did not complete within 1s`); state is polled anyway (1 s). Without hook state a pane
  counts as Claude by command/title (`title_fallback`).
- One instance per tab; instances in INACTIVE tabs get no Tab/PaneUpdate/Visible events (stale data; after a
  reload also no ModeUpdate, so the session name comes from `get_session_list()`). Only the LEADER (lowest
  live claude-tabs plugin pane id in a fresh `get_session_list()` snapshot) dings, auto-switches,
  acknowledges Done -> Idle, assigns characters and deletes state of closed panes. Only the focused instance
  runs the fast timer (spinner, blink). `get_focused_pane_info()` returns the tab ID while the manifest is
  keyed by POSITION; mapped through `TabInfo`.
- `get_session_list()` snapshots lag behind Tab/PaneUpdate (server refreshes session info on a ~1 s tick),
  so an event's tabs/panes win over the snapshot for `EVENT_HOLD_MS` (3 s); `plugin_ids`/session always come
  from the snapshot. `render_key` starts with `A<active tab_id>`; `leader_tick` interval is 2 s for the drawn
  instance (`last_render` < 3 s) as for the leader. Unit test only for the hold rule.
- Permission prompt: zellij 0.45.1 draws it in the 1-row bar ("... Allow? (y/n)") and HOLDS BACK every event
  (Timer included) until answered. The pane stays selectable until PermissionRequestResult (Granted or
  Denied) or the first Tab/Pane/ModeUpdate/RunCommandResult, then `set_selectable(false)` once; zellij
  returns focus to the terminal. `perms` starts false and turns true on those events (a synchronous host
  call such as `get_session_list` PANICS when denied, so `fetch_snapshot`/`in_focused_tab` are gated). Denied
  = bar shows "claude-tabs: permissions denied (README: Troubleshooting)"; the next session asks again. A
  granted PermissionRequestResult IS re-sent to every instance on `start-or-reload-plugin`. With
  `keybinds clear-defaults=true` the 1-row bar cannot be focused to answer: use `setup.sh --permissions`.
- Running agents = UNION of hook markers and `claude-agents.sh <transcript>/subagents` (agent-<id>.jsonl
  modified < 10 min whose last MESSAGE record is not an assistant end_turn/stop_sequence/refusal and not a user
  `[Request interrupted by user...]`; newest 40 of last 30 min; trailing non-message records skipped, last 16
  lines read). A hand-back arrives as SubagentStop only, the main thread then runs turns without
  UserPromptSubmit (hence the `tool` hook), SubagentStart is unreliable. The focused instance and the leader
  poll (`ask_agents`, 3 s); results carry a timestamp (`task_at`) and are ignored after `AGENTS_FRESH_S`
  (10 s). Each poll result is also written to `<pane>.agents` (atomic) and every instance unions a fresh one.
- `SubagentHandback` (Claude Code 2.1.301+): an agent can end with an assistant `tool_use` SubagentHandback +
  the user `tool_result`, no end_turn; the tool_result after a handback call counts as finished, a later user
  line (resume) is running again.
- Killed agents: `TaskStop` (or a user kill) ends the transcript with a user record `[Request interrupted by
  user]` and sends NO SubagentStop. `clear` (PostToolUse) of `TaskStop` removes `<pane>.sub/<task_id>`; every
  Stop first runs `claude-agents.sh --finished <subagents> <pane>.sub` and drops markers whose transcript is
  finished and not older than the marker (a resumed agent's new marker stays). NOT verified: the real
  PostToolUse payload of TaskStop (assumed `tool_input.task_id` = agent id; a named agent's name would not
  match its marker, the Stop reconcile still catches it); a kill from the UI task list.
- A SubagentStop with empty `agent_type` and no SubagentStart (internal helper, ~2 s after the main Stop)
  counts as activity only with a marker or a non-empty `agent_type`.
- Ding only on a real finish: a Stop while agents run is `done <ns> sub` (still Working); after the last one
  stops wait `subagent_grace_s` (8), then ding. Markers older than `subagent_stale_s` (900, hook env
  `ZELLIJ_CLAUDE_STALE_S`) are ignored, > 6 h dropped. `working_hold_s` (20): any hook event newer than the
  last state change keeps the working look. No ding for the ACTIVE tab unless `ding_when_focused` or the
  finish was `away` (OS focus). `ding_with_subagents` = ding on every Stop.
- Attention: hooks `PermissionRequest ask`, `Notification notify` (script keeps permission_prompt /
  elicitation_dialog / elicitation_url_dialog, ignores idle_prompt etc.), `PreToolUse tool` with tool_name
  AskUserQuestion/ExitPlanMode write the OVERLAY `<pane>.attn` = `<ns>[ away] ag=<main|agent_id|->` (not a
  state token: working/done/grace logic untouched). Created with `ln` (one of concurrent raises wins); an
  existing overlay is not rewritten, so the signals of one request ding once. `<pane>.attn-seen` is a
  tombstone of the last raise (10 s) that stops a late duplicate event re-raising after a `clear`. Plugin:
  `St::Attn` wins in `eff_state`, `?` inverted underlined badge + blink; the leader dings once per ns
  (`attn_reacted`, `attention_repeat_s`), no auto-switch; on the ACTIVE tab it is deleted at once unless
  `away` (kept until os-focus says focused). Cleared by `clear` (PostToolUse/Failure), a tool event,
  SubagentStop of the asker, UserPromptSubmit, Stop, start, end; an event with an `agent_id` clears only an
  overlay with the same id or `-`. A Stop within 60 s of a MAIN-agent overlay (`ag=main|-`) writes `done ...
  quiet` (no second ding). Denied permissions fire no hook (next tool event / Stop clears). Overlays older
  than 12 h are ignored. NOT verified with real payloads (Notification/PermissionRequest `agent_id` for
  subagents unconfirmed).
- `<state root>/font` (`1`/`0`): the hook runs `fc-list 'WC3 Icons' family` in the BACKGROUND when the file is
  missing or older than 10 min (touched first so parallel hooks do not re-check); no fc-list = 0. Darwin: 1 when
  `wc3-icons-mac.ttf` exists in `~/Library/Fonts` or `/Library/Fonts` (no fc-list; `install-mac.sh` removes the
  file after installing). Depends on the hook's $HOME. Plugin `race_font auto` = this flag; `on`/`off` force.
  The flag only says "installed", not "this terminal draws it".
- Duplicate hooks (plugin + settings hooks; Claude Code dedupes only identical command strings): every event
  except tool/clear does `mkdir <pane>.once.<cksum of event+JSON>`; the loser exits; any other event of the
  pane removes old markers. Temp files are `.$$`-suffixed. `end` and the plugin's `cleanup_stale` remove
  markers (and `.sess`).
- Characters: `assets/wc3-icons/characters.tsv` (race, character, codepoint U+1FA70..U+1FAD6, clips,
  icon_file, role, sex, theme; 92 rows) is embedded with `include_str!` (REBUILD the wasm after changing it;
  empty icon_file = race glyph). `Rytíř` uses the footman icon; `Rytíř 2 (knight rider on horse)` reuses the
  slot freed by `Vesničani`, which has no row while its clips are excluded. A character is the filename part
  before the first " - " in `sounds/wc3/<race>/`; "Trollí čarodějnice - lékařka" contains " - " itself, so
  the scan (`play-phrase.sh --list` = `race<TAB>prefix`) is resolved against the tsv (`resolve_scan`);
  unknown names stay as listed (race icon, hashed accent). Pool = option `characters` (comma separated) else
  every scanned character with clips (all tsv rows before the first scan; assignment waits for it), narrowed
  by `races`. Least-used first, then diversity-weighted random among ties; stored per stable `tab_id`
  (`create_new`, first writer wins). Existing tabs are never reassigned.
- Diversity (`assign_chars`, `diverse_pick`, option `diversity`, default on): tsv columns `role` (hero/unit),
  `sex` (m/f, `-` = unknown/group), `theme` (coarse voice/style family: demon, arcane, knight, nature,
  undead, troll, orcish, shaman, goblin, machine, rider, naga, nightelf; `-` = none) filled by hand from the
  WC3 unit types (theme is a heuristic). Weight per candidate = 1/(1+P)^2, P = sum over the session's tabs of
  race 1.0 + role 0.5 + sex 0.5 + theme 1.5 (unknown never matches), x2 for the adjacent tab; x0.4 if in the
  last 8 of `recent-characters`; divided by (candidates of its race)^0.5. Hard: candidates with the same race
  AND (same role or same theme) as an adjacent tab are dropped when any other remains. Falls back to a
  uniform pick on a zero/NaN weight total; `assign_chars` skips an empty candidate set. Constants `DIV_*` in
  `src/main.rs`. Tests: `cargo test diversity`, `sh tests/diversity.sh` (1000 simulated sessions, fixed
  seed). NOT verified in a live session.
- Control: `claude-ctl.sh character <name>` / `race <race>` / `ding [character]` / `reshuffle`; pipe names
  `claude_character`, `claude_race`, `claude_ding`. play-phrase.sh matches `"<Character> - "*` in all race
  folders; a character without clips plays any clip; nothing = exit 3. The library is rescanned every 30 s
  by the LEADER only, which publishes the result in `<state root>/library-<session>` (first line = unix
  secs, then the `--list` lines); followers read it (fresh < 100 s) and scan themselves only when it is
  missing/stale after a 15 s grace.
- Tint = `character_accent` (`src/main.rs`): the `RACES` colour in HSL, hue +-12 deg, lightness +-8 %
  (+-12 % grey neutral), saturation +-8 %, spread per tsv index (golden-ratio sequences). `tint_colors`
  (`src/tab.rs`): inactive = accent scaled to `race_dim` (0.1-1, default 0.4), text mixed to just 4.5:1;
  active = accent +15 %, black/white text, bold. Theme-independent. Plain inactive tabs (no tint): zellij
  maps old-style themes' `fg` to `ribbon_unselected.background` (bright), so `plain_colors` (`src/tab.rs`)
  blends it toward `text_unselected.background` by option `plain_dim` (0.1-1, default 0.18 = median
  luminance of inactive Claude tabs at race_dim 0.4, test-guarded; 1 = off) and picks text at just 4.5:1;
  not applied to active/blinking/bell/session-dimmed tabs or 8-bit themes. Working = italic marker,
  finished-unseen = inverted badge, blink phase `(ms / blink_ms) % 2`, `blink_timeout_s` (600). Spinner
  frames `· ✢ ❃ ✶ ✻ ✽` ping-pong (Claude Code 2.1.303 uses ✳ at the third frame; swapped for ❃ because U+2733 is an emoji codepoint; the title match in main.rs still looks for ✳). Options `race_icons/race_font/race_tab_colors/
  race_dim/race_all_tabs` keep their old names.
- Initials badge (no colour font: `race_font` off, or auto with font flag 0): 2-cell chip instead of the glyph
  (`badge_candidates`/`badge_table`, `badge_colors`): initials of words 1+2 (single word: letters 1+2),
  width-1 letters only, uppercase. Collisions resolved over the WHOLE tsv in tsv order (same badge in every
  session): first holder keeps it, later ones take word-1 initial + the next free letter. All 92 unique.
  Tab widths equal the font mode. Unknown library names are appended after the tsv.
- Icon codepoints: private-use (U+F0100..), U+E0100, CJK U+20000.. and unassigned U+1F7EC/U+1FA00 are drawn
  by kitty as 1-cell MONOCHROME masks; only emoji-presentation codepoints get the colour bitmap, hence
  characters use U+1FA70..U+1FAD6 (the first 92 width-2 codepoints per kitty; `tab.rs` pads one space
  after any icon, `is_icon_font_char`). Side effect: WC3 icons replace those emoji in the terminal.
  `is_icon_font_char` assumes width 2 for U+1FA70..1FAFF though unicode-width 0.2.2 reports 1 for some unused
  codepoints.
- Playback (play-phrase.sh): every ding goes through one mkdir mutex `<root>/play.lock` (stale = dead owner
  pid or 5 s; bounded wait). `<root>/playing` = `<pid> <comm> <start ms> <char>` (trusted only while that pid
  lives with that comm and < 2 min). One clip at a time: same character within `dup_window_ms` (2000) of the
  previous start = dropped; a ding while a clip plays WAITS (queue of one in `<root>/queued`, <= 15 s,
  `CLAUDE_TABS_QUEUE_MAX_S`), a further one is dropped; nothing is killed. Exit 0 = played or dropped, 3 =
  nothing to play / no player / command not found, else the player's status. `ding()` in main.rs runs
  `ding_command` / packaged ding.wav ONLY on exit 3. One line per attempt in `ding.log` (trigger
  finish|attention|ctl, who, char, clip, outcome, queued_ms, lead_in_ms; rotated at 100 KB). `lead_in_ms`
  (default 0, max 3000): a generated silent WAV (`<root>/silence-<ms>.wav`) played right before the clip to
  wake a sleeping sink. `max_clip_s` (seconds, fractional ok, 0/junk = none): `ding()` passes it as env
  `ZELLIJ_CLAUDE_MAX_CLIP_S` (only when set; never for `--list`); play-phrase.sh drops longer clips, a
  character without a short one plays its shortest. Players: autodetect afplay, pw-play, paplay, ffplay,
  aplay; extensions per player (`claude_exts`: afplay no Ogg, pw-play/paplay no m4a, aplay wav only).
- Tab label `SSH: <host>`: while an `ssh` runs in a tab's main pane the label is that instead of the folder.
  Detection (each 2 s poll, drawn instance only): `get_pane_pid` of the main pane + ONE
  `sh -c "ps -Aww -o pid=,ppid=,args= | cut -c1-300"` (portable Linux/macOS); `find_ssh_hosts` walks the
  pane's descendants (the pane process itself counts), takes the newest ssh not below another ssh
  (ProxyJump's `ssh -W` child is ignored), `ssh_host` parses argv (skips options and their arguments,
  strips `user@`, `ssh://`, `:port`; host as written, no DNS). Folder and ssh labels go through the same
  `observe_name` debounce (2 s), applied when the ps answer arrives (`apply_labels`); user-typed names still
  win; ps failure = folder. Unit tests `ssh_hosts_from_argv`, `ssh_found_among_descendants`. Caveats: args
  are split on whitespace (quoted args with spaces mis-parse); ssh run by a tool inside a Claude tab (child
  of claude) also shows; mosh/et not handled.
- Tab labels: folder of the pane's ROOT process (`get_pane_cwd`, 2 s debounce); zellij's own auto-names
  follow the FOREGROUND process (a Bash `cd /tmp && ...` renamed tabs), so they are ignored with `auto_name`.
  User names (changed during/within 3 s of interactive RenameTab) stick, stored in `names/`;
  `zellij action rename-tab` is not detected as a user name. `[ ]` other-clients cursors only with
  `show_other_clients`.
- Character name next to the clock: option `clock_character` (default `true`, needs `clock`). The active
  tab's character (`tab_chars`) is drawn as ` Name ` immediately left of the clock, bold, text in the active
  tab's colour on the clock's background (`text_unselected.background`): with `race_tab_colors` +
  `race_icons` the tab background from `tint_colors(char_style accent, active=true)`, else
  `ribbon_selected.background`; dimmed sessions: plain clock colours, italic. Contrast against the bar
  background is not enforced. `State::clock_char_text` (main.rs) -> `line::tab_line(.., clock_char)` ->
  `RightSideElementsBuilder::build` (line.rs). Parenthesized text is stripped from the name (`strip_parens`, clock only). Name is cut to the free cells with an ellipsis
  (`truncate_cells`, display-cell width) and dropped when fewer than 4 cells remain; it never pushes the
  clock out. NOT verified visually (needs a tab with an assigned character). If the name lags after closing
  a tab by more than ~2 s, suspect zellij's own event ordering.
- compact-bar parity verified in an isolated session (overflow, fullscreen, sync, narrow widths, mouse); not
  verified: non-NORMAL mode colours, bell markers, multi-client, nested sessions, clipboard hints.

## OS focus ("away")
- zellij-tile 0.45 has NO plugin event for terminal/OS focus, so `os-focus.sh [SESSION]` asks the desktop:
  focused window pid+title (macOS lsappinfo/osascript, Hyprland hyprctl, Sway swaymsg+jq, KDE Wayland
  kdotool, X11 xdotool/xprop; X11 tools never trusted on Wayland; GNOME Wayland = unknown). Decision: title
  `<session> | ...` (how zellij 0.45 titles the host terminal) = focused; else the focused pid is an
  ancestor of a client of the session (Linux: `ss -xpH` server-side sockets
  `.../contract_version_1/<session>` give the client pid; else every `zellij` client from ps) and the title
  does not name another running session. Every external call capped at 1 s (`run_timeout`).
- Stale title: zellij only rewrites the host title when a pane title changes, so after switching session
  INSIDE one client the title names the OLD session. A title naming another session only counts when a
  client of THAT session still lives under the focused pid (`ss_clients`/`under_fpid`); a title naming this
  session counts only when it has an attached client; the ps fallback is used only without `ss`. Without
  `ss` (macOS) the stale title is still trusted: unverified, open.
- Hook: Stop writes `done <ns>[ sub][ away][ quiet]` (`away` only for a clear `unfocused`; env
  `ZELLIJ_CLAUDE_OS_FOCUS=0` skips; the wait for the helper is bounded). Plugin (`ding_when_os_unfocused`,
  default true): an `away` finish on the ACTIVE tab dings, is not acknowledged and blinks; the leader polls
  `os-focus.sh <session>` every 2 s (`ask_os_focus`) and acknowledges once a result other than `unfocused`
  arrives after the finish. A session with no client has no active tab, so every finish there dings.
- Verified: isolated session with stub backend and player, real Hyprland read-only, helper unit tests with
  PATH stubs for every backend. NOT verified: real macOS, Sway, X11, KDE, GNOME; terminals other than
  kitty; the `ding_when_os_unfocused false` path.

## Sounds
- `sounds/wc3/<race>/*.flac`: 795 active clips of 91 characters (human 186, orc 164, undead 120, nightelf
  128, naga 37, demon 19, neutral 141; per character = `clips` column of `characters.tsv`),
  `_excluded/<race>/` 44 (never played; `_`/hidden folders are never races), `manifest.tsv` (file, status,
  exclusion_reason, race, character, transcript, duration, video, times, lufs, flags), README, NOTICE
  (Blizzard content, included at the owner's decision). FLAC level 8, mono 48 kHz s16, -20 LUFS (tp-limited
  ones down to -22, a few -23), TP <= -2 dBTP; ~135 MB total, names NFC. Pit Lord and the villagers are
  fully excluded (no active clip; `characters.tsv` clips 0); long clips (> 8 s) are allowed where the owner
  restored them; merged clips over 8 s stay excluded ('too long').
- Masters and the whole production pipeline are OUTSIDE this repo in `<workspace>` (yt-dlp of playlist
  PLQNF-ZjhDFeDZaqd5qCr6dE60e1XXgNnP, demucs vocals, GPU whisper.cpp Vulkan in `<gpu-whisper dir>`, python
  venv `<venv>`, LLM-planned cuts in `scripts/opus/`, validation in `work/validation/`). Never commit
  masters/raw/vocals.
- Pipeline per video (`scripts/opus/`, run with the venv python): `analyze.py` (candidates) -> plan
  `work/opus/<vid>.plan` (ops S/J/D/C/T/TS/TE/X/R/OK, see `build.py` docstring) -> `build.py <vid>`
  (refine cuts, cutcheck gate, render) -> `verify.py <vid>` (q5 re-transcription) -> `finalize.py`
  (rebuilds `clips/` + manifest; replaced files go to `work/opus/superseded/`) -> package. Owner transcript
  edits: `work/edit/clips.md` -> `interpret.py` -> `apply_edit_plan.py` (`gen` -> `work/opus/text_edits.tsv`,
  time-keyed ops merge/split/move/text that `build.py` applies after the plan; `check` = dry run with
  cutcheck; `pins` = owner status via the pinned include/exclude lists); `verify_textedit.py` re-transcribes
  changed clips. Exclusions are heuristic apart from the owner's edit; prune by regenerating `clips.md`
  (`gen_clips.py --force`, edit, `interpret.py`) or via the pinned lists.
- Cut validation: `cutcheck.py` runs inside `build.py` before rendering and STOPS the build on errors
  (`CUTCHECK=warn` renders anyway). An edge passes if it is in a pause (energy <= max(peak-35, min(local
  floor+6, peak-25)) dB), at the bottom of a pause on a noise bed (<= peak-25 and within 3 dB of the
  +-0.15 s minimum) or on a soft slope (reaches the pause level within 0.10 s, never above peak-20); the
  first/last Whisper word must not start >0.25 s before the start / after the end. Exceptions are accepted
  per line with `OK <anchor> reason` (`OK X<start>` for X lines). It also warns when a cut is >0.10 s from the
  bottom of the adjacent gap (58 active clips). `refine()`: a split with no >=0.1 s pause looks up to 0.6 s
  past the next line's DTW start for a real pause; `extend_edges()` moves every start back / end forward to
  the bottom of the gap (<= 0.5 s, never into the neighbouring line's onset/offset or word starts, plan
  TS/TE stay exact). Two boundaries could not be cut ('magii | to se dá snést' split; moving 'Nechte to na
  mě.' to the next Trollí lovec lebek line): the words stay in the previous clip.
- Re-package after library changes: `python3 <workspace>/scripts/package_dotfiles.py --dry --plan
  /tmp/plan.tsv`, diff it against `sounds/wc3/manifest.tsv` (sorted), then run without `--dry` (target:
  `--out DIR`, `$WC3_SOUNDS_OUT`, default this checkout's `sounds/wc3`). Exclusions come from
  `work/validation/exclude.tsv`, mapped through `work/opus/fixes_rename.tsv` (action `edges` = same line
  with new edges, exclusion inherited; recut/split/merged = re-evaluated by the rules), then
  `exclude_pinned.tsv` / `include_pinned.tsv` override; a real run rebuilds `_excluded` from these, so
  manual moves must go into those lists.
- `sounds/ding.wav`: derived from a downloaded chime `stu9-chime-356831.mp3` (not in the repo, licence
  unknown): `ffmpeg -y -i stu9-chime-356831.mp3 -filter_complex "[0:a]atrim=0:1.7,asetpts=PTS-STARTPTS,acrossover=split=3000:order=4th[lo][hi];[hi]afade=t=out:st=0.1:d=0.3:curve=qsin[hf];[lo][hf]amix=inputs=2:normalize=0,afade=t=in:d=0.005,afade=t=out:st=0.7:d=1.0:curve=cub,volume=6dB,alimiter=limit=0.66:attack=2:release=50:level=disabled" -ar 44100 -ac 1 -c:a pcm_s16le sounds/ding.wav`
  (no afftdn: it adds a low swoosh).

## Icon font (`assets/wc3-icons/`)
- `wc3-icons.ttf`: CBDT colour bitmap font, family `WC3 Icons`: the 92 character glyphs U+1FA70..U+1FAD6
  (plus the 7 race glyphs U+1F7E0..U+1F7E6, 2 cells wide, used for characters without an icon). Installed on
  Linux only on request by `fontconfig/install.sh` (= `setup.sh --font`). macOS: see below (sbix builds).
  Terminals need BOTH ranges mapped (U+1F7E0-1F7E6 race fallbacks, U+1FA70-1FAD6 characters).
- Rebuild: `cd assets/wc3-icons && python3 build.py` (defaults: FAMILY "WC3 Icons", PPEM 96, S 144, width 112
  (ASPECT 0.7778), BY 116, reference cells 14x36; needs python3 with fonttools + Pillow). The icon is
  centre-cropped to ASPECT (never squashed), 4 px frame cropped only when `has_frame()` sees it. kitty
  scales a strike by font_px/PPEM (font_size 18 = 24 px -> x0.25) and shrinks it by whole px once it exceeds
  the 2-cell box, so the bitmap must scale to EXACTLY 28x36 (S 144/PPEM 96 gives 36 px; 145/97 left a
  half-covered top row). BY moves the icon 0.25 px/unit (115 = top row partial, 117 = bottom row partial).
  Retune: S/PPEM so S*font_px/PPEM == cell height and W*font_px/PPEM <= 2*cell width (CBDT limits:
  height/width <= 255, BY <= 127). Test builds need another family name AND a private FONTCONFIG_FILE +
  XDG_DATA_HOME.
- `wc3-icons-wide.ttf` (family `WC3 Icons Wide`): `FAMILY="WC3 Icons Wide" ASPECT=1.0345 BY=92 PAD_X=4 python3
  build.py wc3-icons-wide.ttf 1212 4 116` (120x116 bitmap, icon 112x116 centred with 4 transparent strike px each
  side = 1 px at ghostty 18, so 1 px gap on all sides there with adjust-cell-height 2; build.py `PAD_X`). For cells
  about 1:2 (ghostty any size, foot/alacritty Paper Mono 18 at 100 % line height; there the icon is 28 px wide in a
  30 px box). Rebuild both after icon changes.
- Terminals (measured in isolated windows + pixel analysis; tables in `assets/wc3-icons/README.md`, config
  lines in README Quick start): kitty needs `symbol_map U+1FA70-U+1FAD6 WC3 Icons` (REQUIRED: kitty 0.49.2
  picks Noto Color Emoji otherwise despite `fc-match`; exact only at 14x36). ghostty ignores fontconfig for
  emoji (needs `font-codepoint-map`) and the font's PPEM/BY/ascent: it fits the bitmap ASPECT into 2 cells x
  the font's NATURAL height, centred (`adjust-cell-height` = background bands; Wide = exact at 0). foot
  (fcft) and alacritty scale by font_px/PPEM unfitted from the baseline (foot clips, alacritty overflows
  upward); foot needs the font in its `font=` fallback list + the und-zsye tag; alacritty needs a fontconfig
  family-append rule (scope it with `<test name="prgname"><string>alacritty</string></test>`). Custom builds
  per cell: README formula. Untested: wezterm/konsole/gnome-terminal/xterm/st. Harness pitfalls: test
  families need unique names, private FONTCONFIG_FILE + XDG_CONFIG_HOME + XDG_CACHE_HOME (scan rules need a
  fresh cache); foot 1.28 uses `[colors-dark]`; ghostty reloads on SIGUSR2, kitty on SIGUSR1, alacritty live.
- macOS (CoreText does NOT draw the CBDT font, boxes appear): `build.py` has `SBIX=1` ->
  `assets/wc3-icons/wc3-icons-mac.ttf` (family `WC3 Icons Mac`, PNG strike PPEM 72, 88x94 px = 44x47 px per 2
  cells on Retina; kitty 0.49.2, Paper Mono 18 + cell_height 110%; recipe + geometry in
  `assets/wc3-icons/README.md`, "macOS font"). `fontconfig/install-mac.sh` (`~/Library/Fonts`, never
  overwrites a foreign file, `--uninstall`, `--check`, drops the font flag, prints the terminal lines; edits no
  terminal config) installs it. Pitfalls: the font needs name IDs 3/4/5/6, else CoreText family lookups
  fail; a per-glyph box outline in `glyf` broke the family; CoreText caches by family/file, so use a new name
  while tuning (a rebuilt font under an old name shows "?" boxes); kitty clips glyphs to the cell, so BY must
  be exact.
- ghostty on macOS (1.3.1): second font `wc3-icons-mac-ghostty.ttf`, family `WC3 Icons Wide` (same family as
  Linux, so the shared dotfiles ghostty config works on both OSes), built `SBIX=1 SBIX_BOX=1 SBIX_DX=48 PPEM=72
  BY=94 ASPECT=0.99` (recipe in README "ghostty on macOS"). ghostty fits the OUTLINE rect into 2 cells x
  face_height and crops the CoreText-drawn bitmap to that canvas; CoreText does not scale `originOffsetY` with
  the font size, so BY must equal S. ghostty pads emoji 2.5 % per side (SharedGrid.zig) and rounds sbix canvases to whole px; the renderer adds
  the shaper's GPOS x offset AFTER that, unclipped, so `SBIX_DX` (GPOS `kern` XPlacement, units/1000 of font px)
  sets the left gap. A font cannot see the size or `adjust-cell-height`, so it is tuned per setting: 48 = 2 px
  left at font-size 21 + `adjust-cell-height = 4` (2 px top/bottom too, icon 50x50; formula + table in the
  icons README). Left gap depends only on the size, the bands only on `adjust-cell-height`, CoreText ignores GPOS
  Device tables: one static font matches at most one N per size (matrix in the icons README).
  kitty: 0 px, 44 px exact. A glyph
  without outline draws nothing; a glyph object shared between names compiles once (others empty); kitty
  shrinks icons that have the box (kitty font has none). A `font-codepoint-map` to a missing family is
  ignored (system emoji), but the last matching line wins even if missing: per-OS lines cannot share one
  config. Map U+1F7E0-U+1F7E6 too. `install-mac.sh` installs both files and removes the earlier name
  `wc3-icons-mac-wide.ttf`. Test ghostty by launching the binary with `--config-default-files=false
  --config-file=...` (`open -g -n -a Ghostty` shows no window). NOT verified: real zellij bar in ghostty,
  other font sizes, non-Retina, `/Library/Fonts`.

## macOS install (short checklist; same route as Linux, README Quick start)
1. Prereqs: `brew install zellij` (0.45.x), Claude Code. No Rust needed (the wasm is prebuilt and tracked);
   sounds use `afplay` (built in).
2. Hooks: `claude plugin marketplace add crossatko/zellij-wc3cz-claude` +
   `claude plugin install zellij-wc3cz@zellij-wc3cz` (once per `CLAUDE_CONFIG_DIR`), then `/reload-plugins`
   or restart every running `claude`.
3. Permissions (optional, skips the in-bar `Allow? (y/n)`):
   `sh ~/.claude/plugins/marketplaces/zellij-wc3cz/setup.sh --permissions` (writes
   `~/Library/Caches/org.Zellij-Contributors.Zellij/permissions.kdl`; `--link` adds the legacy alias path).
4. Bar: `zellij -n ~/.claude/plugins/marketplaces/zellij-wc3cz/layouts/claude.kdl` (or copy it to
   `~/.config/zellij/layouts/claude.kdl` and `zellij -n claude` / `default_layout "claude"`).
5. Colour icons (optional; without them initials badges): `sh ~/.claude/plugins/marketplaces/zellij-wc3cz/
   fontconfig/install.sh` (-> `install-mac.sh`: `~/Library/Fonts/wc3-icons-mac.ttf` + `wc3-icons-mac-ghostty.ttf`),
   then the terminal line it prints: kitty `symbol_map U+1FA70-U+1FAD6,U+1F7E0-U+1F7E6 WC3 Icons Mac`; ghostty
   `font-codepoint-map = U+1FA70-U+1FAD6=WC3 Icons Wide` and the same for `U+1F7E0-U+1F7E6`. Restart the terminal fully.
6. Updating: `claude plugin marketplace update zellij-wc3cz` + `claude plugin update zellij-wc3cz@zellij-wc3cz`,
   reload every running `claude`; if the bar still shows initials after a font install, delete
   `<state root>/font` (`$TMPDIR/zellij-$(id -u)/zellij-claude/font`). Hooks running from an OLD cache copy
   (`${CLAUDE_PLUGIN_ROOT}` = old sha) keep rewriting the old flag logic (a pre-Mac-font version always
   writes 0) every 10 min.
7. Check: README "macOS checklist". Steps 2-5 + updating verified on one Mac (Darwin 25, zellij 0.45.1,
   kitty colour portraits in the real bar); hooks/afplay/os-focus runtime not yet.

## macOS (code paths)
- Done for it: claude-lib.sh (Homebrew PATH, state-root candidates, `run_timeout` timeout/gtimeout/perl,
  `now_ns` without `%N`), no GNU-only flags, `get_pane_cwd` instead of /proc, chess pieces for human/orc
  symbols (U+2694/U+2620 may render as emoji), `uname -s` via run_command, dev.sh fswatch or stat polling.
  The font flag on Darwin was checked with a scratch TMPDIR (1 with the font installed).
- Risks (unverified): `get_pane_cwd`, `getconf DARWIN_USER_TEMP_DIR`, BSD find/od/grep flags, perl
  Time::HiRes, spinner glyphs drawn as emoji (marker uses text-only U+273B), afplay FLAC (10.13+, from memory), two zellij servers with
  differing TMPDIR.

## Integration with a user's dotfiles (what such a setup needs)
Example paths; the package works without any of it (plugin + layout route), this is for a dotfiles repo that
manages zellij/terminal config.
- zellij `config.kdl`: an alias `claude-tabs location="file:~/.config/zellij/plugins/claude-tabs/claude-tabs.wasm" { ...options }`
  (the permission grant is keyed by this exact expanded path, so keep it stable), `default_layout "claude"`
  with a copy of `layouts/claude.kdl` as `layouts/claude.kdl` (other layouts that want the bar use
  `plugin location="claude-tabs"`). `plugins/claude-tabs` is a host-local symlink to the checkout created by
  `setup.sh --link`; keep `plugins/` git-ignored in the dotfiles.
- Dotfiles installer: call `sh "$CLAUDE_TABS_DIR/setup.sh"` (default checkout dir, overridable) when it exists,
  else print a hint pointing at the compact layout as fallback.
- Fallback wrapper: KDL has no conditionals, so a `zellij` shell function (zsh) checks that
  `${ZELLIJ_CONFIG_DIR:-~/.config/zellij}/plugins/claude-tabs/claude-tabs.wasm` exists; if not it adds
  `--layout compact` (and an `nvim-compact` variant of other layouts, `options --default-layout compact` for
  `attach -c/-b`) only for commands that can start a session; everything else passes through.
- Terminal icon fonts (both codepoint ranges U+1F7E0-1F7E6 and U+1FA70-1FAD6 must be mapped; restart the
  terminal fully): kitty `symbol_map ... WC3 Icons` in a Linux-only include and `... WC3 Icons Mac` in a
  macOS include (kitty `include ${KITTY_OS}.conf` pattern); ghostty `font-codepoint-map = <range>=WC3 Icons
  Wide` (same family on both OSes, but the last matching line wins even if the family is missing, so one
  shared config works only because both OSes install a font with that family). Harmless if the font is
  not installed.
- Claude Code: enable the plugin `zellij-wc3cz@zellij-wc3cz` in each config dir's `settings.json` (the CLI
  writes `extraKnownMarketplaces`/`enabledPlugins` on `plugin install`); no `claude-hook.sh` entries in
  settings.json, else every hook runs twice.
- Machines without the checkout/font (fresh box): the wrapper falls back to the compact bar; without the
  font the plugin shows initials badges.

## Known issues / caveats
- With two package copies the plugin sticks to the first confirmed one (not necessarily the newest);
  `package_dir` forces one. Followers adopt "confirmed" from the leader's library file, not from a scan.
- Bluetooth wake-up clipping of the first ms of a voice line is a hypothesis; test with `lead_in_ms 300` and
  `ding.log`.
- The font flag says "installed", not "this terminal draws it": every terminal needs its config line
  (README); icons are unverified on untested terminals. Such users set `race_font off`.
- Once after a reload one finish produced two identical dings (two instances acted); not reproduced.
- After a reload, tabs whose last Stop was `done ... sub` show the spinner for the grace period.
- The subagent count depends on hooks/transcripts; a missed stop (other than TaskStop/kill) sticks until the
  next Stop finds the transcript finished, stale (900 s), end or pane close.
- Unexplained: a ~10 s gap before the flash after a finish was seen once.
- Unverified: interactive `/plugin` UI; the `n` (deny) permission flow beyond an isolated session; several
  tabs open while the permission prompt is pending, terminals narrower than the prompt; edge cases (HOME
  with spaces, narrow terminal).
