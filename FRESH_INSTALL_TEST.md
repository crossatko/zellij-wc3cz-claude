# Fresh-install test: handoff for the agent on the second machine

Goal: prove that a user with NO prior setup can install this package from the README alone, and that
everything works, without editing any of their existing config files. You are the "new user". Do the
install exactly as the README's Quick start says, do not improvise, and write down every place where
the README was wrong, incomplete or unclear. Fix the docs/code for what you find and push.

Read first: `README.md` (Quick start, Uninstall, Troubleshooting) and `AGENTS.md` (architecture, hooks,
state files, testing rules). This file only adds the test plan.

## State of the project (handed over from the first machine)

- Everything is committed and pushed to `main`. Library: 795 active clips, 44 excluded, 91 characters with
  clips (Pit Lord is fully excluded by the owner's decision, villagers too). Prebuilt `claude-tabs.wasm` is
  tracked; no Rust toolchain is needed for the test.
- Design in one paragraph: a zellij tab-bar plugin; every Claude Code tab gets one WC3 character (icon or
  initials badge, tint, Czech voice line when the agent finishes or needs attention). Hooks come from the
  Claude Code plugin in this repo (`.claude-plugin/`, `hooks/hooks.json`); the hook tells the plugin where
  the package lives (state file `pkgdir`), so no symlink or `package_dir` option is needed. Colour icons
  need an optional font (`fontconfig/install.sh`); without it the bar shows initials badges.
- Verified on machine 1 (Linux, kitty, Hyprland, zellij 0.45.1, PipeWire): the plugin, hooks via
  `claude -p --plugin-dir`, marketplace add/install from a LOCAL directory and a git URL in a scratch
  config dir, the permission prompt flow (empty cache, y/n, pre-granted, reload), fontconfig rules via
  `fc-match`, attention alerts and playback queue with simulated hooks and stub players.
- NEVER verified (this is what you are here for): install from the real GitHub repo through
  `claude plugin marketplace add crossatko/zellij-wc3cz-claude`; the interactive `/plugin` UI; the
  layout path `~/.claude/plugins/marketplaces/zellij-wc3cz/...` on a clean machine; first-run permission
  prompt in a real terminal on a clean zellij cache; colour icons in a real terminal window other than
  kitty; macOS; real Claude Code hook payloads for permission prompts / AskUserQuestion on a second
  machine; Bluetooth wake-up clipping of the first ms of a voice line (hypothesis, see `lead_in_ms`).

## Preconditions to check and record (do not install anything system-wide without noting it)

`zellij --version` (needs 0.45.x), `claude --version`, `jq` (optional, only for legacy settings
editing), a sound player (`pw-play`/`paplay`/`ffplay`/`aplay`/`afplay`), the terminal in use, `fc-list`.
Confirm the machine has NO earlier install: no `~/.claude/plugins/marketplaces/zellij-wc3cz`, no
`claude-hook.sh` entries in any `~/.claude*/settings.json`, no `~/.config/zellij/plugins/claude-tabs`,
no `wc3-icons.ttf` in the fonts dir, no `60-wc3-icons.conf`, no old grants in zellij's `permissions.kdl`
for this plugin. If something is there, the owner decides: ask before deleting; take backups.

## Test procedure

1. Run the Quick start step 1 (Claude plugin) literally. Record the exact output and any error. Check
   `claude plugin list` shows it enabled, and that hooks are active in a NEW `claude` session
   (`/hooks` or by effect below). Note how long the marketplace clone takes and its size.
2. Quick start step 2 (layout): start the shipped layout via the documented command. Check the bar
   renders, the permission prompt appears in the 1-row bar, it can be answered with `Alt+Down`/click + `y`,
   and the bar becomes unfocusable afterwards. Test `n` once in a throwaway session (bar says denied, no
   crash, a new session asks again).
3. In that session start `claude` in two tabs. Check, in order: (a) tabs get different characters, label
   shows the folder, spinner while working; (b) `ls /tmp/zellij-$(id -u)/zellij-claude/<session>/` shows
   pane state files and `pkgdir` points at the plugin's copy (the marketplace clone or the plugin cache);
   (c) a finishing agent in an inactive tab plays exactly ONE clip of that tab's character and shows the
   finished badge; the active+focused tab does not ding; (d) ask Claude something that triggers a
   permission prompt and an `AskUserQuestion` in an inactive tab: attention marker `?` and one voice line;
   the marker clears after answering; (e) `ding.log` in the state dir shows one line per ding.
4. Without the colour font: initials badges are shown (2 letters, unique per character). Then run
   `sh <plugin dir>/fontconfig/install.sh` (it only writes two files it owns), restart the terminal, and
   check colour icons (kitty expected; other terminals per the README table: record what you see). Then
   `--uninstall` and confirm the files are gone.
5. Uninstall section of the README: run it literally on this machine at the end (marketplace removal,
   permission grants, state dir, fontconfig) and verify nothing of ours remains (`claude plugin list`, the
   fonts dir, `~/.config/fontconfig/conf.d/`, `/tmp/zellij-*/zellij-claude`). Check the Claude settings
   files were never modified by the package (`git diff`-style comparison of `~/.claude*/settings.json`
   before and after: it must be byte-identical).
6. Edge checks if time allows: narrow terminal (prompt cut off), two Claude config dirs, a path with
   spaces in `HOME`, running `setup.sh` with no flags (must change nothing: it prints status),
   `dev.sh` refuses to reload a session that has no such plugin pane.

## What to produce

- A results section appended to `AGENTS.md` ("Fresh-install test, <date>"): what was run, what passed,
  what failed, with exact commands and short outputs; verified vs still unverified. Keep it short and
  factual, update stale statements in place.
- Fix README/code for everything you hit (wrong command, missing step, unclear wording, bug). Small
  commits, push to `main`. Rebuild the wasm only if you change `src/` (`cargo build --release --target
  wasm32-wasip1`, copy over `claude-tabs.wasm`, `cmp`); then reload sessions only in your own test
  sessions.
- Report to the owner: a short verdict (does the quick start work as written, yes/no), the list of
  deviations, and anything that needs their decision.

## Rules for the whole session

- The owner allows committing and pushing small logical commits. NO AI/Claude attribution anywhere
  (no Co-Authored-By, no "Generated with", nothing in code comments, notes or commit messages).
- No real personal setup details in tracked files or commits (hostnames, IPs, emails, absolute
  `/home/<user>` paths, real zellij session names, workspace paths): placeholders only.
- Never inject synthetic keystrokes into the owner's terminals. Do not screenshot the whole screen,
  only your own test window. Do not kill processes by name; kill only your own by exact PID.
- Don't modify the owner's existing config files (zellij, kitty, Claude settings, fontconfig) except
  what the documented install steps do; take a backup first and say so. No secrets in logs or notes.
- The voice-line library was produced on machine 1 in an out-of-repo pipeline workspace that does NOT
  exist on this machine; do not try to re-cut audio here. If you find a bad clip, note its file name in
  the results and leave it.
