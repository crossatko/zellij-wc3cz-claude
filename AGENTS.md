# claude-tabs (zellij-wc3cz-claude): notes for agents

State and history of this package, for whoever works on it next. User-facing docs: `README.md`.
Keep entries short and factual, update in place, no secrets, no personal setup details (hostnames, home
paths, session names, monitors). Agents commit small logical commits and push; no AI attribution anywhere.

## What it is
A zellij 0.45 tab-bar plugin (Rust -> wasm32-wasip1, zellij-tile 0.45.1) that replaces the stock
`compact-bar` (ported from zellij v0.45.1 `default-plugins/compact-bar`: `line.rs`, `tab.rs`, clipboard
hints; tooltips dropped) and adds Claude Code awareness: `✳` marker + Claude Code's spinner while working,
running-subagent count (`✳2`), finished-unseen badge + blink, attention alerts (`?` badge), one Warcraft III
CHARACTER per Claude tab (icon + tint + voice, 91 characters with clips), a Czech WC3 voice line of that character when
an agent finishes or asks (fallback `sounds/ding.wav`), cwd-based tab labels, clock. Linux verified; macOS
written blind.

## Layout of the repo
- `src/main.rs` (state, characters, sounds, hooks, timers), `src/tab.rs` (ribbon render, tint), `src/line.rs`
  (bar layout, ported), `src/clipboard_utils.rs`; `Cargo.toml`/`Cargo.lock`.
- `claude-tabs.wasm`: TRACKED prebuilt (what layouts load; rebuild + commit it after every source change).
  `target/` is git-ignored.
- Packaging: `.claude-plugin/plugin.json` + `marketplace.json` (repo root = marketplace = plugin, name
  `zellij-wc3cz`), `hooks/hooks.json`, `fontconfig/` (`install.sh`, generated `60-wc3-icons.conf`),
  `setup.sh` (optional helper), `dev.sh` (build + hot reload), `layouts/claude.kdl` (compact layout + bar).
- Shell side (POSIX sh, bash 3.2 / BSD-safe, all locate siblings via their own resolved dir):
  `claude-hook.sh` (Claude Code hook), `claude-agents.sh` (running agents from transcripts),
  `claude-ctl.sh` (ding/character/race/reshuffle via `zellij pipe`), `play-phrase.sh` (clip picker,
  `--list`, `--ding`), `os-focus.sh` (OS window focus), `claude-lib.sh` (sourced helpers, mode 644).
- `sounds/ding.wav`, `sounds/wc3/` (library), `assets/wc3-icons/` (icon font + build, `characters.tsv`).
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
- Route: Claude Code plugin for the hooks + a zellij layout for the bar; everything else optional.
  `setup.sh` without options = status/dry run, writes nothing. Opt-in: `--font`/`--font-uninstall`,
  `--permissions`, `--build`, `--migrate-to-plugin`, `--link` (legacy alias path), `--legacy-settings-hooks`
  (old `--hooks`, deprecated; `--no-hooks` accepted, no-op); targets `--settings FILE` (repeatable),
  `--all-claude-dirs`, else `$CLAUDE_SETTINGS`, `$CLAUDE_CONFIG_DIR/settings.json`, `~/.claude/settings.json`.
  `--help` prints only the header. Status reports DOUBLE REGISTRATION when plugin and settings hooks coexist;
  `--migrate-to-plugin` removes exactly our `claude-hook.sh <arg>` handlers (other hooks/keys kept,
  backup `<file>.bak-claude-tabs`, idempotent).
- Plugin: the name must not start with `claude-` (reserved), so plugin and marketplace are `zellij-wc3cz`,
  marketplace entry `source: "./"`. No `version` in plugin.json on purpose (version = git SHA; validate
  warns, passes). `hooks/hooks.json` has the 11 events, no matchers, no timeouts, command
  `"${CLAUDE_PLUGIN_ROOT}/claude-hook.sh" <arg>`:
  SessionStart `start`, UserPromptSubmit `working`, PreToolUse `tool`, PermissionRequest `ask`,
  Notification `notify`, PostToolUse and PostToolUseFailure `clear`, Stop `done`, SubagentStart
  `substart`, SubagentStop `substop`, SessionEnd `end`. A plugin hook and an identical settings hook BOTH run.
- Where things land (verified with Claude Code 2.1.303 in a scratch HOME): plugins root
  `$CLAUDE_CONFIG_DIR/plugins` (default `~/.claude/plugins`). Git marketplace = full clone at
  `plugins/marketplaces/zellij-wc3cz/` (stable path with the wasm, what the layout loads) + a COPY per
  version at `plugins/cache/zellij-wc3cz/zellij-wc3cz/<12-char sha>/` (`${CLAUDE_PLUGIN_ROOT}`, changes on
  update). A local-path marketplace loads in place. Disk: clone + each cached version carry the sound
  library. Marketplace add+install verified from a local dir and a local git URL; NOT verified: from
  GitHub, the interactive `/plugin` UI.
- zellij plugin loading (0.45.1 source): `file:/abs`, `file:~/...` (tilde + env expansion), relative
  `file:x` = joined with the session/layout cwd; `https://` URLs are downloaded ONCE and never refreshed.
  `zellij --layout X` inside a session or with `-s` ADDS tabs; `zellij -n X` starts a new session. Plugin
  options go in the layout's plugin block. Shipped layout loads
  `file:~/.claude/plugins/marketplaces/zellij-wc3cz/claude-tabs.wasm` (verified in an isolated session).
- `fontconfig/install.sh` (Linux/BSD; macOS/others print "not supported, initials badges", exit 0): copies
  the font to `${XDG_DATA_HOME:-~/.local/share}/fonts/wc3-icons.ttf` (never overwrites a foreign
  `wc3-icons.ttf`), writes `${XDG_CONFIG_HOME:-~/.config}/fontconfig/conf.d/60-wc3-icons.conf` GENERATED
  from the installed font's charset (repo copy = `install.sh --print-conf`; regenerate after font
  changes: `sh fontconfig/install.sh --print-conf > fontconfig/60-wc3-icons.conf`), fc-cache, prints proof.
  Refuses to overwrite a conf without its marker; `--uninstall`, `--check`. Rules apply ONLY to the
  character icon codepoints (circle emoji etc. untouched): (1) `emoji` alias prefers WC3 Icons (kitty looks
  up emoji-presentation chars as family "emoji"+color, then takes the first candidate covering the char);
  (2) weak append to every pattern (never beats a requested family; a strong append hijacked
  `fc-match monospace`); (3) per-codepoint `charset contains` -> prepend_first. Verified in a sandbox with
  `fc-match` and kitty's own matcher (headless); NOT verified: rendering in real kitty/alacritty/foot
  windows by this route. Side effect: `fc-match emoji` without a charset answers WC3 Icons. The `font`
  flag only says the font is installed, not that the terminal draws it.
- Sandbox rule: tests of setup.sh / plugin install need HOME/XDG_*/CLAUDE_CONFIG_DIR in a scratch dir.
  `claude plugin validate .` should only warn about the missing version.

## Build / dev / test
- Toolchain: Arch `sudo pacman -S rust rust-wasm`; macOS `brew install rustup && rustup-init -y &&
  rustup target add wasm32-wasip1`. Build: `cargo build --release --target wasm32-wasip1`, copy
  `target/wasm32-wasip1/release/claude-tabs.wasm` over `claude-tabs.wasm` (`dev.sh`, `setup.sh --build`).
  Unit tests: `cargo test --target x86_64-unknown-linux-gnu`.
- Hot reload: `./dev.sh [-s SESSION | --all] [--watch] [-p URL | -a [NAME]]` = build, copy, then
  `zellij --session S action start-or-reload-plugin X` (reloads every per-tab instance of the plugin loaded
  as exactly X). X that matches nothing LAUNCHES a new plugin pane in the focused tab (it once split a live
  Claude pane), so dev.sh first checks `list-panes -a` (columns split by two spaces; column 7 = the location
  string the pane was loaded with) and only reloads with a target present in the session: autodetect =
  alias `claude-tabs`, else `file:<checkout>/claude-tabs.wasm`, else REFUSE with rc 1 (also for `-p`/`-a`
  values that are not present). Layouts loading another path/symlink/marketplace clone need `-p` with that
  exact string. Verified in isolated sessions (alias, exact path, symlinked path -> refused): pane count
  unchanged each time. A reload picks up the new wasm, NOT new plugin options (fixed at session start).
  `--watch`: inotifywait, fswatch, entr, else 1 s polling.
- Test in an ISOLATED session with its own `ZELLIJ_CONFIG_DIR`/`XDG_CACHE_HOME`, a test wasm copy and a stub
  player; drive it with simulated hook calls (`ZELLIJ=0 ZELLIJ_SESSION_NAME=<test> ZELLIJ_PANE_ID=N sh
  claude-hook.sh done </dev/null`). Never kill or reshuffle someone's live sessions. No screenshots of the
  whole screen: text/SGR dumps, or crop only the test window; floating test windows must not steal focus
  (check the focused window before and after).
- Logs: `grep claude-tabs /tmp/zellij-$(id -u)/zellij-log/zellij.log` (`scan found [...]`,
  `tab N -> character`, `ding_command failed`). Hook debug: `touch <state root>/debug` logs every event + JSON
  to `<state root>/debug.log`. Ding log: `<state root>/ding.log`.
- Do not `zellij pipe` this plugin bare: it blocks until every per-tab instance answers; `claude-ctl.sh`
  caps it with a timeout and sends the payload on stdin.
- Sanity: all scripts pass `sh -n` and `bash --posix -n`.

## How it works (findings)
- State root = the dir the plugin sees as `/tmp` = `<temp_dir()>/zellij-<uid>`; `claude_state_root` picks
  `$TMPDIR`, `/tmp` or `getconf DARWIN_USER_TEMP_DIR`, the first containing `zellij-<uid>/zellij-log`.
  Files under `zellij-claude/<session>/`: `<pane>` (`idle|working|done <ns> [sub] [away] [quiet]`),
  `<pane>.sub/<agent_id>` markers, `<pane>.ev` (activity), `<pane>.sess` (agent transcript dir),
  `<pane>.attn` / `<pane>.attn-seen` (attention, below), `<pane>.agents` (shared agent scan, below),
  `characters/<tab_id>`, `names/<tab_id>`,
  `last-<character>` (last clip, name sanitised); at the root also `pkgdir`, `font`, `play.lock`, `playing`,
  `queued`, `ding.log`.
- Hooks are a silent no-op outside zellij. Poke = backgrounded `zellij pipe --name claude_poke` (logs a
  harmless `CliPipe did not complete within 1s`); state is polled anyway (1 s). Without hook state a pane
  counts as Claude by command/title (`title_fallback`).
- One instance per tab; instances in INACTIVE tabs get no Tab/PaneUpdate/Visible events (stale data; after a
  reload also no ModeUpdate, so the session name comes from `get_session_list()`). Only the LEADER (lowest
  live claude-tabs plugin pane id in a fresh `get_session_list()` snapshot) dings, auto-switches,
  acknowledges Done -> Idle, assigns characters and deletes state of closed panes. Only the focused instance
  runs the fast timer (spinner, blink). `get_focused_pane_info()` returns the tab ID while the manifest is
  keyed by POSITION; mapped through `TabInfo`.
- `get_session_list()` snapshots lag behind Tab/PaneUpdate: the server refreshes session info on a ~1 s tick
  (`~/.cache/zellij/.../session_info/<s>/session-metadata.kdl` mtimes all share one sub-second phase).
  `fetch_snapshot` used to overwrite `tabs`/`manifest` unconditionally, and the leader fetches from
  `refresh()` inside the TabUpdate handler itself (every 2 s), so right after a tab switch the bar of the
  tab switched TO highlighted the tab switched FROM again (user report). Now an event's tabs/panes win for
  `EVENT_HOLD_MS` (3 s); `plugin_ids`/session always come from the snapshot. Root cause from code + timing,
  NOT reproduced live (unit test only for the hold rule).
- Permission prompt: zellij 0.45.1 draws it in the 1-row bar ("... Allow? (y/n)") and HOLDS BACK every event
  (Timer included) until answered. The pane stays selectable until PermissionRequestResult (Granted or
  Denied) or the first Tab/Pane/ModeUpdate/RunCommandResult, then `set_selectable(false)` once; zellij
  returns focus to the terminal. `perms` starts false and turns true on those events (a synchronous host
  call such as `get_session_list` PANICS when denied, so `fetch_snapshot`/`in_focused_tab` are gated). Denied
  = bar shows "claude-tabs: permissions denied (README: Troubleshooting)"; the next session asks again. A
  granted PermissionRequestResult IS re-sent to every instance on `start-or-reload-plugin` (verified).
  Verified in an isolated headless session (fresh cache -> prompt, focus, `y` -> grant, `n` -> no panic).
  Not verified: several tabs open while the prompt is pending, terminals narrower than the prompt.
- Running agents = UNION of hook markers and `claude-agents.sh <transcript>/subagents` (agent-<id>.jsonl
  modified < 10 min whose last MESSAGE record is not an assistant end_turn/stop_sequence/refusal and not a user
  `[Request interrupted by user...]`; newest 40 of last 30 min). Real data:
  a hand-back arrives as SubagentStop only, the main thread then runs turns without UserPromptSubmit (hence
  the `tool` hook), SubagentStart is unreliable. The focused instance and the leader poll (`ask_agents`, 3 s);
  results carry a timestamp (`task_at`) and are ignored after `AGENTS_FRESH_S` (10 s) (else a stale list in an
  inactive leader kept finished tabs Working). Each poll result is also written to `<pane>.agents` (atomic)
  and every instance unions a fresh one: before, a just-focused non-leader dropped its own stale list, so a
  scan-only agent showed "spinner + count" in another tab's bar but not in its own for 1-3 s.
- `SubagentHandback` (Claude Code 2.1.301+, seen in real transcripts of the second config dir): an agent can
  end with an assistant `tool_use` SubagentHandback + the user `tool_result`, no end_turn. claude-agents.sh
  read that as running for 10 min (mtime window): Working + "1" on the tab and the Stop deferred as `sub`
  (7 of 8 such transcripts). Now the tool_result after a handback call counts as finished; a later user line
  (resume) is running again. Verified: tests/subagents.sh and all 235 real transcripts older than 15 min read
  finished.
- Killed agents (Claude Code 2.1.30x, real session): `TaskStop` (or a kill by the user) ends the agent's
  transcript with a user record `[Request interrupted by user]` and sends NO SubagentStop, so its marker
  stayed and the tab showed Working / "1 agent running" and deferred the ding until `subagent_stale_s`
  (900 s) expired it (seen: marker 23:18:08 -> plugin removed it 15 min later, ding 10 s after). Fix
  (script-only): `clear` (PostToolUse) of `TaskStop` removes `<pane>.sub/<task_id>`; every Stop first runs
  `claude-agents.sh --finished <subagents> <pane>.sub` and drops markers whose transcript is finished and not
  older than the marker (a resumed agent's new marker stays). Verified: `tests/subagents.sh` (simulated
  payloads) and `--finished` on 237 real transcripts (the killed one detected). NOT verified: the real
  PostToolUse payload of TaskStop (assumed `tool_input.task_id` = agent id; a named agent's name would not
  match its marker, the Stop reconcile still catches it); a kill from the UI task list (assumed same transcript).
- Internal helper (previous real-run finding, 2.1.304): a SubagentStop with empty `agent_type` and no
  SubagentStart ~2 s after the main Stop. It touched `.ev`, so `working_hold_s` showed the spinner again after
  the finish. Now a substop counts as activity only with a marker or a non-empty `agent_type`. Verified with
  simulated payloads only.
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
  quiet` (no second ding). Docs: denied permissions fire no hook (next tool event / Stop clears). Overlays
  older than 12 h are ignored. Verified in an isolated session with simulated payloads; NOT with real
  payloads (Notification/PermissionRequest `agent_id` for subagents unconfirmed).
- `<state root>/font` (`1`/`0`): the hook runs `fc-list 'WC3 Icons' family` in the BACKGROUND when the file is
  missing or older than 10 min (touched first so parallel hooks do not re-check); Darwin or no fc-list = 0.
  Depends on the hook's $HOME. Plugin `race_font auto` = this flag; `on`/`off` force.
- Duplicate hooks (plugin + settings hooks; Claude Code dedupes only identical command strings): every event
  except tool/clear does `mkdir <pane>.once.<cksum of event+JSON>`; the loser exits; any other event of the
  pane removes old markers. Temp files are `.$$`-suffixed. `end` and the plugin's `cleanup_stale` remove
  markers (and `.sess`). Verified with stubs (10x concurrent Stop pairs: one poke) and in an isolated
  session (concurrent Stops from two install dirs: 1 clip per round).
- Characters: `assets/wc3-icons/characters.tsv` (race, character, codepoint U+1FA70..U+1FAD6, clips,
  icon_file; 92 rows) is embedded with `include_str!` (REBUILD the wasm after changing it; empty icon_file =
  race glyph). `Rytíř` uses the footman icon; `Rytíř 2 (knight rider on horse)` reuses the slot freed by
  `Vesničani`, which has no row while its clips are excluded. A character is the filename part before the
  first " - " in `sounds/wc3/<race>/`; "Trollí čarodějnice - lékařka" contains " - " itself, so the scan
  (`play-phrase.sh --list` = `race<TAB>prefix`) is resolved against the tsv (`resolve_scan`); unknown names
  stay as listed (race icon, hashed accent). Pool = option `characters` (comma separated) else every scanned
  character with clips (all tsv rows before the first scan; assignment waits for it), narrowed by `races`.
  Least-used first, then diversity-weighted random among ties (below); stored per stable `tab_id` (`create_new`, first writer wins).
  Diversity (`assign_chars`, `diverse_pick`, option `diversity`, default on): tsv columns `role` (hero/unit),
  `sex` (m/f, `-` = unknown/group), `theme` (coarse voice/style family: demon, arcane, knight, nature,
  undead, troll, orcish, shaman, goblin, machine, rider, naga, nightelf; `-` = none) are filled by hand from
  the WC3 unit types (sex only where certain; theme is a heuristic, NOT derived from voice actors, none
  known). Weight per candidate = 1/(1+P)^2, P = sum over the session's tabs of race 1.0 + role 0.5 + sex 0.5 +
  theme 1.5 (unknown never matches), x2 for the adjacent tab (tab position distance 1); x0.4 if in the last 8
  of `/tmp/zellij-claude/recent-characters` (cross-session, 16 lines); divided by (candidates of its race)^0.5
  so 2-character races are not starved. Hard: candidates with the same race AND (same role or same theme) as
  an adjacent tab are dropped when any other remains. Constants `DIV_*` in `src/main.rs`. Verified: unit
  tests (`cargo test diversity`, `sh tests/diversity.sh`; 1000 simulated sessions of 2-8 tabs, fixed seed):
  0 adjacent hard clashes, adjacent same-theme 20 vs 255 for uniform random, races 291/955/490/860/777/887/737
  (demon/human/naga/neutral/nightelf/orc/undead) vs 109/1192/223/889/781/1043/760. NOT verified in a live
  session. Existing tabs are never reassigned; `claude_race`/`claude_character` pipes are unchanged.
  `claude-ctl.sh character <name>` / `race <race>` / `ding [character]` / `reshuffle`; pipe names
  `claude_character`, `claude_race`, `claude_ding`. play-phrase.sh matches `"<Character> - "*` in all race
  folders; a character without clips plays any clip; nothing = exit 3. The library is rescanned every 30 s
  by the LEADER only, which publishes the result in `<state root>/library-<session>` (first line = unix
  secs, then the `--list` lines); followers read it (fresh < 100 s) and scan themselves only when it is
  missing/stale after a 15 s grace. Verified: 3 tabs -> 1 `--list` call per 30 s, pkgdir flips between two
  package copies caused no extra scans.
- Tint = `character_accent` (`src/main.rs`): the `RACES` colour in HSL, hue +-12 deg, lightness +-8 %
  (+-12 % grey neutral), saturation +-8 %, spread per tsv index (golden-ratio sequences). `tint_colors`
  (`src/tab.rs`): inactive = accent scaled to `race_dim` (0.1-1, default 0.4), text mixed to just 4.5:1;
  active = accent +15 %, black/white text, bold. Theme-independent.
  Plain inactive tabs (no tint): zellij maps old-style themes' `fg` to `ribbon_unselected.background` (bright), so
  `plain_colors` (`src/tab.rs`) blends it toward `text_unselected.background` by option `plain_dim` (0.1-1, default
  0.18 = median luminance of inactive Claude tabs at race_dim 0.4, test-guarded; 1 = off) and picks text at just 4.5:1; not applied to active/blinking/bell/session-dimmed tabs or 8-bit themes. Working = italic marker, finished-unseen
  = inverted badge, blink phase `(ms / blink_ms) % 2`, `blink_timeout_s` (600). Spinner frames
  `· ✢ ✳ ✶ ✻ ✽` ping-pong (Claude Code 2.1.303). Options `race_icons/race_font/race_tab_colors/race_dim/
  race_all_tabs` keep their old names.
- Initials badge (no colour font: `race_font` off, or auto with font flag 0): 2-cell chip instead of the glyph
  (`badge_candidates`/`badge_table`, `badge_colors`): initials of words 1+2 (single word: letters 1+2),
  width-1 letters only, uppercase. Collisions resolved over the WHOLE tsv in tsv order (same badge in every
  session): first holder keeps it, later ones take word-1 initial + the next free letter. All 92 unique.
  Tab widths equal the font mode. Unknown library names are appended after the tsv.
- Icon codepoints: private-use (U+F0100..), U+E0100, CJK U+20000.. and unassigned U+1F7EC/U+1FA00 are drawn
  by kitty as 1-cell MONOCHROME masks; only emoji-presentation codepoints get the colour bitmap, hence
  characters use U+1FA70..U+1FAD6 (the first 92 width-2 codepoints per kitty; `tab.rs` pads one space
  after any icon, `is_icon_font_char`). Side effect: WC3 icons replace those emoji in the terminal.
  Verified by screenshot in an isolated kitty (colour, whole faces, aligned, tinted backgrounds); not in
  the real zellij bar. `is_icon_font_char` assumes width 2 for U+1FA70..1FAFF though unicode-width 0.2.2
  reports 1 for some unused codepoints.
- Playback (play-phrase.sh; verified with a stub player and a null sink): every ding goes through one mkdir
  mutex `<root>/play.lock` (stale = dead owner pid or 5 s; bounded wait). `<root>/playing` = `<pid> <comm>
  <start ms> <char>` (trusted only while that pid lives with that comm and < 2 min). One clip at a time:
  same character within `dup_window_ms` (2000) of the previous start = dropped; a ding while a clip plays
  WAITS (queue of one in `<root>/queued`, <= 15 s, `CLAUDE_TABS_QUEUE_MAX_S`), a further one is dropped;
  nothing is killed. Exit 0 = played or dropped, 3 = nothing to play / no player / command not found, else
  the player's status. `ding()` in main.rs runs `ding_command` / packaged ding.wav ONLY on exit 3. One
  line per attempt in `ding.log` (trigger finish|attention|ctl, who, char, clip, outcome, queued_ms,
  lead_in_ms; rotated at 100 KB). `lead_in_ms` (default 0, max 3000): a generated silent WAV
  (`<root>/silence-<ms>.wav`) played right before the clip to wake a sleeping sink. Null sink: clip
  bit-exact, onset delayed by lead_in + ~100-170 ms. Bluetooth wake-up clipping the first ms is only a
  HYPOTHESIS: test with `lead_in_ms 300` and `ding.log`.
- Tab labels: folder of the pane's ROOT process (`get_pane_cwd`, 2 s debounce); zellij's own auto-names
  follow the FOREGROUND process (a Bash `cd /tmp && ...` renamed tabs), so they are ignored with `auto_name`.
  User names (changed during/within 3 s of interactive RenameTab) stick, stored in `names/`;
  `zellij action rename-tab` is not detected as a user name. `[ ]` other-clients cursors only with
  `show_other_clients`.
- compact-bar parity verified in an isolated session (overflow, fullscreen, sync, narrow widths, mouse); not
  verified: non-NORMAL mode colours, bell markers, multi-client, nested sessions, clipboard hints.

## OS focus ("away")
- zellij-tile 0.45 has NO plugin event for terminal/OS focus (`FocusGained/FocusLost` exist only in the
  nested-session protocol), so `os-focus.sh [SESSION]` asks the desktop: focused window pid+title (macOS
  lsappinfo/osascript, Hyprland hyprctl, Sway swaymsg+jq, KDE Wayland kdotool, X11 xdotool/xprop; X11 tools
  never trusted on Wayland; GNOME Wayland = unknown). Decision: title `<session> | ...` (how zellij 0.45
  titles the host terminal) = focused; else the focused pid is an ancestor of a client of the session (Linux:
  `ss -xpH` server-side sockets `.../contract_version_1/<session>` give the client pid; else every `zellij`
  client from ps) and the title does not name another running session. Every external call capped at 1 s
  (`run_timeout`).
- Stale-title fix: zellij only rewrites the host title when a pane title changes, so after switching session
  INSIDE one client the title still names the OLD session and the answer was `unfocused` forever (finished
  tab blinked forever). Now a title naming another session only counts when a client of THAT session still
  lives under the focused pid (`ss_clients`/`under_fpid`); a title naming this session counts only when it
  has an attached client; the ps fallback is used only without `ss`. Verified in isolated sessions with a
  stub window-manager command. Without `ss` (macOS) the stale title is still trusted: unverified, open.
- Hook: Stop writes `done <ns>[ sub][ away][ quiet]` (`away` only for a clear `unfocused`; env
  `ZELLIJ_CLAUDE_OS_FOCUS=0` skips; the wait for the helper is bounded). Plugin (`ding_when_os_unfocused`,
  default true): an `away` finish on the ACTIVE tab dings, is not acknowledged and blinks; the leader polls
  `os-focus.sh <session>` every 2 s (`ask_os_focus`) and acknowledges once a result other than `unfocused`
  arrives after the finish. A session with no client has no active tab, so every finish there dings.
- Verified: isolated session with a stub backend and stub player (away -> 1 clip + blink; focus back ->
  acked in seconds; focused finish -> no clip; unknown backend -> no clip), a real Hyprland read-only
  (focused via title, detached session -> unfocused), helper unit tests with PATH stubs for every backend.
  NOT verified: real macOS, Sway, X11, KDE, GNOME; terminals other than kitty; the
  `ding_when_os_unfocused false` path.

## Sounds
- `sounds/wc3/<race>/*.flac`: 795 active clips of 91 characters (human 186, orc 164, undead 120, nightelf
  128, naga 37, demon 19, neutral 141; verified against `manifest.tsv` and the files), `_excluded/<race>/` 44 (never played; `_`/hidden folders are never races),
  `manifest.tsv` (file, status, exclusion_reason, race, character, transcript, duration, video, times,
  lufs, flags), README, NOTICE (Blizzard content, included at the owner's decision). FLAC level 8, mono
  48 kHz s16, -20 LUFS (tp-limited ones down to -22, a few restored ones -23), TP <= -2 dBTP; ~135 MB total,
  names NFC.
  Per-character active counts = `clips` column of `assets/wc3-icons/characters.tsv` (embedded in the wasm:
  rebuild after changing it).
- Masters and the whole production pipeline are OUTSIDE this repo in `<workspace>` (the owner's pipeline workspace; yt-dlp of
  playlist PLQNF-ZjhDFeDZaqd5qCr6dE60e1XXgNnP, demucs vocals, GPU whisper.cpp Vulkan in
  `<gpu-whisper dir>`, python venv `<venv>`, LLM-planned cuts in `scripts/opus/`,
  validation in `work/validation/`). Never commit masters/raw/vocals.
- Pipeline per video (`scripts/opus/`, run with the venv python): `analyze.py` (candidates) -> plan
  `work/opus/<vid>.plan` (ops S/J/D/C/T/TS/TE/X/R/OK, see `build.py` docstring) -> `build.py <vid>`
  (refine cuts, cutcheck gate, render) -> `verify.py <vid>` (q5 re-transcription) -> `finalize.py`
  (rebuilds `clips/` + manifest; replaced files go to `work/opus/superseded/`) -> package.
- Cut validation (2026-10-20): `cutcheck.py` runs inside `build.py` before rendering and STOPS the build
  on errors (`CUTCHECK=warn` renders anyway). An edge passes if it is in a pause (energy <= max(peak-35,
  min(local floor+6, peak-25)) dB), at the bottom of a pause on a noise bed (<= peak-25 and within 3 dB of
  the +-0.15 s minimum) or on a soft slope (reaches the pause level within 0.10 s, never above peak-20);
  the first/last Whisper word must not start >0.25 s before the start / after the end. Exceptions are
  accepted per line with `OK <anchor> reason` (`OK X<start>` for X lines); 19 such lines exist (DTW word
  starts in the pause, flat noise/growl beds, the Gyrokoptéra 'mysli'/'Ouha' junction). It also warns
  when a cut is >0.10 s from the bottom of the adjacent gap (58 active clips left, capped by the next line
  or the 0.5 s limit). `refine()`: a split with no >=0.1 s pause looks up to 0.6 s past the next line's
  DTW start for a real pause instead of cutting at the quietest frame (that cut 'zmrzlinou' in half);
  `extend_edges()` moves every start back / end forward to the bottom of the gap (<= 0.5 s, never into
  the neighbouring line's onset/offset or word starts, plan TS/TE stay exact).
- 2026-10-20 re-cut: before, ~28 % of the active clips had a clipped tail/onset or a cut in sound
  (226/798 with a cutcheck error or valley warning), now 58/799 warnings, 0 unaccepted errors. 327 clips
  only got longer edges (530 are byte-identical); fixed by plan: Lord Garithos split off by one word,
  Trollí čarodějnice 'zmrzlinou' cut mid-word (+ 'Hm' was its end: now 'To vypadá dobře'), two Naga
  myrmidon X ends, Minometčíci 'Aha, jasně,' moved to the next line, Lovkyně and Alchemist half-sentence
  splits merged, Gyrokoptéra 'mysli', Krvavý mág / Rexxar / Druid drápu onsets, Pilot noise tail (2.4 s).
  Every plan edit is marked `2026-10-20` in the plans. 'Železný trol...' is now 8.15 s and excluded by
  the >8 s rule; 'Rexxar - Zvířata kolem mě...' keeps its old end to stay <= 8 s.
- Re-package after library changes: `python3 <workspace>/scripts/package_dotfiles.py --dry --plan
  /tmp/plan.tsv`, diff it against `sounds/wc3/manifest.tsv` (sorted), then run without `--dry` (target:
  `--out DIR`, `$WC3_SOUNDS_OUT`, default this checkout's `sounds/wc3`). Exclusions come from
  `work/validation/exclude.tsv`, mapped through `work/opus/fixes_rename.tsv` (action `edges` = same line
  with new edges, exclusion inherited; recut/split/merged = re-evaluated by the rules), then
  `exclude_pinned.tsv` / `include_pinned.tsv` override; a real run rebuilds `_excluded` from these, so
  manual moves must go into those lists.
- Backup of the workspace state before the 2026-10-20 re-cut: `<workspace>/work/backup-20261020-cutfix/`.
- 2026-10-20 owner transcript edit (the owner edited every line in `work/edit/clips.md`, kept as
  `clips.edited.md`; `interpret.py` -> `edit_plan.applied.*`): applied by `scripts/opus/apply_edit_plan.py`
  (`gen` -> `work/opus/text_edits.tsv`, time-keyed ops merge/split/move/text that `build.py` applies after the
  plan; `check` = dry run with cutcheck; `pins` = owner status via the pinned include/exclude lists). Result
  838 clips at that time (785 active / 53 excluded; later owner-review commits restored 9 clips >8 s and split one: see HEAD counts above), 743 byte-identical, 95 new cuts; merges keep the parts' outer cuts,
  split/move cuts were checked against f16 word timings and energy (`probe.py`, `align.py`). Every changed or
  restored clip was re-transcribed with f16 (`verify_textedit.py`, `work/textedit/verify.tsv`). Not applied:
  the 'magii | to se dá snést' split and moving 'Nechte to na mě.' to the next Trollí lovec lebek line (no
  pause anywhere near the word boundary: the words stay in the previous clip and its text says so); villagers
  and Pit Lord are fully excluded, their lines were left as they were (no merge/text fix). Merged clips over
  8 s stay excluded ('too long'): Přízračný chodec, Akolyta, Smrtonožka, Naga royal guard, Lord Garithos,
  Kapitán; restored long clips are active by the owner's decision. `Rytíř (nejistý)` is now
  `Rytíř 2 (knight rider on horse)` (the horse-riding knight, owner), restored. Backup of the state before:
  `<workspace>/work/backup-20261020-textedit/`.
- Exclusions are heuristic apart from the owner's edit; prune by moving lines in a regenerated
  `work/edit/clips.md` (`gen_clips.py --force`, edit, `interpret.py`) or by the pinned lists.
- Players: autodetect afplay, pw-play, paplay, ffplay, aplay; extensions per player (`claude_exts`:
  afplay no Ogg, pw-play/paplay no m4a, aplay wav only).
- `sounds/ding.wav`: derived from a downloaded chime `stu9-chime-356831.mp3` (not in the repo, licence
  unknown): `ffmpeg -y -i stu9-chime-356831.mp3 -filter_complex "[0:a]atrim=0:1.7,asetpts=PTS-STARTPTS,acrossover=split=3000:order=4th[lo][hi];[hi]afade=t=out:st=0.1:d=0.3:curve=qsin[hf];[lo][hf]amix=inputs=2:normalize=0,afade=t=in:d=0.005,afade=t=out:st=0.7:d=1.0:curve=cub,volume=6dB,alimiter=limit=0.66:attack=2:release=50:level=disabled" -ar 44100 -ac 1 -c:a pcm_s16le sounds/ding.wav`
  (no afftdn: it adds a low swoosh).

## Icon font (`assets/wc3-icons/`)
- `wc3-icons.ttf`: CBDT colour bitmap font, family `WC3 Icons`: the 92 character glyphs U+1FA70..U+1FAD6
  (plus the 7 race glyphs U+1F7E0..U+1F7E6, 2 cells wide, used for characters without an icon). Installed on
  Linux only on request by `fontconfig/install.sh` (= `setup.sh --font`). macOS: flag always 0 (CoreText
  has no CBDT; an sbix variant would be needed).
- Rebuild: `cd assets/wc3-icons && python3 build.py` (defaults: FAMILY "WC3 Icons", PPEM 96, S 144, width 112
  (ASPECT 0.7778), BY 116, reference cells 14x36; needs python3 with fonttools + Pillow). The icon is
  centre-cropped to ASPECT (never squashed), 4 px frame cropped only when `has_frame()` sees it. Why these
  numbers: kitty scales a strike by font_px/PPEM (font_size 18 = 24 px -> x0.25) and shrinks it by whole px
  once it exceeds the 2-cell box, so the bitmap must scale to EXACTLY 28x36; the old 145/PPEM 97 gave 35.88
  px = a half-covered top row (thin line above every icon). BY moves the icon 0.25 px/unit (115 = top row
  partial, 117 = bottom row partial). Verified at 14x36: 0 gaps, stacked lines seamless. Retune: S/PPEM so
  S*font_px/PPEM == cell height and W*font_px/PPEM <= 2*cell width (CBDT limits: height/width <= 255,
  BY <= 127). Test builds need another family name AND a private FONTCONFIG_FILE + XDG_DATA_HOME.
- Other terminals (measured, README has the table): kitty OK (other cell sizes undistorted but smaller,
  only 14x36 pixel exact). alacritty: no symbol_map, uses the font only if a fontconfig rule appends it, draws
  unfitted, needs `font.offset.y`. ghostty: ignores fontconfig for emoji codepoints, needs
  `font-codepoint-map` for the icon codepoints, scales to the natural cell. wezterm/foot/konsole/
  gnome-terminal/xterm/st: untested. Mitigations: fontconfig rules, ghostty `font-codepoint-map`, or
  `race_font off` (README).

## macOS (written without a Mac; unverified)
- Done for it: claude-lib.sh (Homebrew PATH, state-root candidates, `run_timeout` timeout/gtimeout/perl,
  `now_ns` without `%N`), no GNU-only flags, `get_pane_cwd` instead of /proc, chess pieces for human/orc
  symbols (U+2694/U+2620 may render as emoji), `uname -s` via run_command, dev.sh fswatch or stat polling.
- Risks: `get_pane_cwd`, `getconf DARWIN_USER_TEMP_DIR`, BSD find/od/grep flags, perl Time::HiRes, ✳/spinner
  glyphs drawn as emoji, afplay FLAC (10.13+, from memory), two zellij servers with differing TMPDIR.
  Checklist: README "macOS checklist".
- Icon font TODO (needs a Mac): the font is CBDT/CBLC; CoreText is believed (NOT tested) to draw only
  `sbix`. Test first: install the font, set `race_font on`, look. If the icons don't render: add an `sbix`
  build (same codepoints, 2 cells wide) to build.py, install it on Darwin, keep `race_font` auto-off until
  verified, tune bitmap size per terminal. Until then macOS uses initials badges.

## Known issues / caveats
- With two package copies the plugin sticks to the first confirmed one (not necessarily the newest);
  `package_dir` forces one. Followers adopt "confirmed" from the leader's library file, not from a scan.
- Bluetooth wake-up clipping of the first ms is a hypothesis; use `lead_in_ms` to test.
- `max_clip_s` (plugin option, seconds, fractional ok, 0/junk = none): `ding()` passes it as env
  `ZELLIJ_CLAUDE_MAX_CLIP_S` (only when set; never for `--list`); play-phrase.sh drops longer clips, a character
  without a short one plays its shortest (Hnus, shortest 3.4 s). Verified in isolated sessions with a stub player:
  `max_clip_s "3"` -> 30 dings of 10 characters, all <= 3 s except Hnus's own shortest; unset -> clips of 8 s and
  25.9 s were picked. Pit Lord has no active clip (character fully excluded, `characters.tsv` clips 0).
- The font flag says "installed", not "this terminal draws it": alacritty/ghostty need extra terminal
  config (README); colour icons are unverified on macOS and other terminals. Such users set `race_font off`.
- Once after a reload one finish produced two identical dings (two instances acted); not reproduced.
- After a reload, tabs whose last Stop was `done ... sub` show the spinner for the grace period.
- The subagent count depends on hooks/transcripts; a missed stop (other than TaskStop/kill, see above) sticks
  until the next Stop finds the transcript finished, stale (900 s), end or pane close.
- Sessions started before a plugin-option change keep the old options; a new session picks them up.
- Voice library: the owner's transcript edit is applied (2026-10-20); two boundaries could not be cut (see Sounds).

## Status / how to resume (2026-10-20)
- Done: standalone git repo; Claude Code plugin + marketplace; opt-in setup.sh; fontconfig installer;
  one WC3 character per tab (icon, tint, voice) with initials fallback; attention alerts; serialised
  playback with ding.log; os-focus stale-title fix; answerable permission prompt; pkgdir path discovery;
  re-cut sound library after the owner's hand edit of the transcripts (795 active / 44 excluded, 91
  characters with clips; long clips are allowed, Pit Lord and the villagers are fully excluded by the
  owner); `max_clip_s` option. The owner listened to the clips flagged as uncertain and found them
  acceptable; the rest was checked with energy + Whisper only.
- A plugin-less config that still has old hooks in Claude settings doubles every hook once the plugin is
  installed: run `setup.sh --migrate-to-plugin`. A script that used to rely on `setup.sh` doing things by
  default now needs the explicit options.
- Fresh-install test done on a second machine (results below). NEXT: macOS and other terminals.
- Open: the Known issues above; test on a Mac and other desktops/terminals; Bluetooth wake-up clipping of the first ms
  of a voice line is an untested hypothesis (try `lead_in_ms`).
- Tests: `sh tests/subagents.sh` (hook/agents scripts, private TMPDIR, stub zellij), `cargo test` (wasm).
- Resume: `sh setup.sh` (status), `./dev.sh -s <isolated session>` (build + reload), `claude plugin
  validate .`; rebuild and commit the wasm after every source change.
- Install audit (Linux, kitty, checkout + Claude Code plugin both present): hooks only via the plugin (cache dir =
  `${CLAUDE_PLUGIN_ROOT}`, writes `pkgdir`), none in settings.json; checkout wasm == fresh build; font + kitty
  `symbol_map` for BOTH U+1F7E0-1F7E6 (race fallbacks, still emitted) and U+1FA70-1FAD6 (characters) are needed.
  Found the installed marketplace clone and cache copy carrying a newer `claude-agents.sh` than git HEAD
  (record-skipping, last-16-lines version); committed after a check on 47 real transcripts (same result as the old version, robust to records appended after the final message).
  Screenshot test in an isolated kitty+zellij session passed (portraits, tint, clock, no prompt).

## Fresh-install test, 2026-10-20

Machine: Linux/Hyprland, PipeWire, kitty 0.49.2, zellij 0.45.1, Claude Code 2.1.302 then 2.1.304, a second
Claude config dir (`CLAUDE_CONFIG_DIR`).
- Run: `claude plugin marketplace add crossatko/zellij-wc3cz-claude` (about 50 s, clone ~270 MB, half `.git`),
  `claude plugin install zellij-wc3cz@zellij-wc3cz` (cache ~140 MB more). Works as written. The clone went
  over HTTPS on the first run and SSH on the second (follows the user's git config; SSH needs a key).
- Deviations, README fixed: (1) with `CLAUDE_CONFIG_DIR` the plugin lives under that dir, so commands and
  `layouts/claude.kdl` now show `${CLAUDE_CONFIG_DIR:-~/.claude}`; each config dir needs its own marketplace
  add/install, one layout serves all (hooks report `pkgdir`). (2) The `claude` CLI itself adds
  `extraKnownMarketplaces`/`enabledPlugins` to settings.json (the package's scripts never do): reworded.
  (3) `fontconfig/install.sh` also creates `conf.d/`: stated. (4) Icons need a full terminal restart, and
  the font flag `<state root>/font` (auto mode, cached 10 min) stayed 0 if a hook ran before the font
  existed: `install.sh` now removes that one file after installing (README says how to do it by hand).
  (5) kitty 0.49.2 picks Noto Color Emoji for U+1FA70..U+1FAD6 despite `fc-match` ranking WC3 Icons first
  (tab showed Noto's mirror ball): `symbol_map U+1FA70-U+1FAD6 WC3 Icons` in kitty.conf is REQUIRED
  (verified in isolated kitty windows with plain printed codepoints and live by the owner). The older
  statement that it is not needed was wrong. (6) With `keybinds clear-defaults=true` there is no
  Alt+hjkl/Alt+Down, the 1-row bar could not be focused to answer the prompt, and the prompt text looked cut
  off on the left in a ~1250 px window (cause not investigated): `setup.sh --permissions` (verified; writes
  `permissions.kdl` atomically) is the recommended path then. (7) A plain `zellij` with
  `default_layout "compact"` never loads the plugin; start `zellij -n claude` or set `default_layout`.
- Claude Code 2.1.30x behaviours found during the run, all handled and pushed (details in "How it works"):
  transcript trailing records (1e38cb2), TaskStop-killed agents leaving a `.sub` marker (7ea1022), helper
  SubagentStop with no marker (ee4a0d0), SubagentHandback transcripts (e1027a5), stale session snapshot
  overwriting TabUpdate (highlight jumped back, per-tab agent count differed; 337d606).
- Verified: marketplace add + install from the real GitHub repo; hooks fire in a new session; layout, bar,
  spinner, colour icons in kitty (after the symbol_map); finish detection after the fixes (owner: "looks
  like it works").
- UNVERIFIED: macOS; other terminals (alacritty, wezterm, foot, ghostty); the interactive `/plugin` UI; the
  `n` (deny) permission flow; Bluetooth lead-in clipping (`lead_in_ms`); a 10 s gap before the flash after a
  finish (unexplained); the two-tab real-payload permission/AskUserQuestion attention flow on this machine;
  edge checks (narrow terminal, HOME with spaces, `setup.sh` with no flags, `dev.sh` refusing foreign
  sessions); uninstall (deliberately skipped, the owner keeps the install).
