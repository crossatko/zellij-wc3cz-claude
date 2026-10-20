#!/bin/sh
# claude-tabs setup, Linux and macOS (POSIX sh, BSD-safe). Run it from anywhere: everything is resolved
# relative to this script, i.e. the package checkout.
#   setup.sh                    full setup (below)
#   setup.sh --no-hooks         same, but leave every Claude settings file alone
#   setup.sh --settings PATH    put the hooks into PATH (repeatable; replaces the default target)
#   setup.sh --all-claude-dirs  put the hooks into every ~/.claude*/settings.json (each ~/.claude* dir,
#                               e.g. ~/.claude, ~/.claude-personal, ~/.claude-work; combinable)
# Hook targets when neither option is given: $CLAUDE_SETTINGS if set (compat), else
# $CLAUDE_CONFIG_DIR/settings.json if CLAUDE_CONFIG_DIR is set, else ~/.claude/settings.json.
# Steps:
#   - makes the helper scripts executable
#   - links <zellij config dir>/plugins/claude-tabs -> this checkout (the path the example config.kdl
#     alias and the plugin's default `package_dir` use); an existing different file/link is left alone
#   - Linux only: installs the WC3 icon font (assets/wc3-icons/wc3-icons.ttf) to ~/.local/share/fonts
#     (macOS: not installed, kitty/CoreText cannot draw CBDT bitmap fonts; race_font is off there)
#   - rebuilds claude-tabs.wasm when a Rust toolchain with the wasm32-wasip1 target is present
#     (Arch: sudo pacman -S rust rust-wasm; macOS: brew install rustup && rustup-init -y &&
#      rustup target add wasm32-wasip1), otherwise keeps the tracked prebuilt claude-tabs.wasm
#   - pre-grants the plugin permissions in zellij's permission cache so there is no prompt
#   - adds the Claude Code hooks to each target settings file, pointing at this checkout's claude-hook.sh
#     (needs jq; per-file backup <file>.bak-claude-tabs, written only when the file changes; hooks that
#     call a claude-hook.sh elsewhere are repointed here; a target whose directory does not exist is
#     skipped, no directory is created)
# Safe to run again (idempotent).
here=$(cd "$(dirname "$0")" && pwd -P)
hooks=1
targets=""      # newline-separated settings files
nl='
'
add_target() { targets="$targets$1$nl"; }
all_dirs=0
while [ $# -gt 0 ]; do
  case "$1" in
    --no-hooks) hooks=0 ;;
    --hooks) hooks=1 ;;   # accepted for compatibility (hooks are on by default)
    --settings) [ $# -ge 2 ] || { echo "--settings needs a path" >&2; exit 2; }; shift; add_target "$1" ;;
    --settings=*) add_target "${1#--settings=}" ;;
    --all-claude-dirs) all_dirs=1 ;;
    -h|--help) sed -n '2,25p' "$0"; exit 0 ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
  shift
done
if [ "$all_dirs" = 1 ]; then
  for d in "$HOME"/.claude*; do
    [ -d "$d" ] && add_target "$d/settings.json"
  done
fi
if [ -z "$targets" ] && [ "$all_dirs" = 0 ]; then
  if [ -n "${CLAUDE_SETTINGS:-}" ]; then add_target "$CLAUDE_SETTINGS"
  elif [ -n "${CLAUDE_CONFIG_DIR:-}" ]; then add_target "${CLAUDE_CONFIG_DIR%/}/settings.json"
  else add_target "$HOME/.claude/settings.json"
  fi
fi

chmod +x "$here"/claude-hook.sh "$here"/claude-ctl.sh "$here"/claude-agents.sh "$here"/play-phrase.sh "$here"/os-focus.sh \
  "$here"/dev.sh "$here"/setup.sh 2>/dev/null

# stable path for config.kdl: <zellij config dir>/plugins/claude-tabs -> this checkout
zcfg=${ZELLIJ_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/zellij}
zcfg=${zcfg%/}
link="$zcfg/plugins/claude-tabs"
if [ -e "$link" ] || [ -L "$link" ]; then
  if [ "$(cd "$link" 2>/dev/null && pwd -P)" != "$here" ]; then
    echo "  claude-tabs: $link exists and is not this checkout; left alone (set package_dir in config.kdl)" >&2
  fi
else
  mkdir -p "$zcfg/plugins" && ln -s "$here" "$link" && echo "  claude-tabs: linked $link -> $here"
fi

font="$here/assets/wc3-icons/wc3-icons.ttf"
case "$(uname -s)" in
  Darwin) echo "  claude-tabs: macOS, WC3 icon font not installed (race_font uses plain symbols)" ;;
  *)
    if [ -f "$font" ]; then
      fdir="${XDG_DATA_HOME:-$HOME/.local/share}/fonts"
      if ! cmp -s "$font" "$fdir/wc3-icons.ttf"; then
        mkdir -p "$fdir" && cp "$font" "$fdir/wc3-icons.ttf" \
          && { command -v fc-cache >/dev/null 2>&1 && fc-cache -f "$fdir"; } \
          && echo "  claude-tabs: installed WC3 icon font to $fdir (reload kitty: ctrl+shift+f5)"
      fi
    else
      echo "  claude-tabs: $font not found, WC3 icon font not installed"
    fi
    ;;
esac

if command -v cargo >/dev/null 2>&1 && rustc --print target-libdir --target wasm32-wasip1 >/dev/null 2>&1 \
   && [ -d "$(rustc --print target-libdir --target wasm32-wasip1 2>/dev/null)" ]; then
  if (cd "$here" && cargo build --release --target wasm32-wasip1 -q); then
    cp "$here/target/wasm32-wasip1/release/claude-tabs.wasm" "$here/claude-tabs.wasm"
    echo "  claude-tabs: rebuilt claude-tabs.wasm"
  else
    echo "  claude-tabs: build failed, keeping the prebuilt claude-tabs.wasm" >&2
  fi
else
  echo "  claude-tabs: no wasm32-wasip1 toolchain, using the prebuilt claude-tabs.wasm"
fi

# zellij's cache dir: ProjectDirs("org", "Zellij Contributors", "Zellij").cache_dir()
case "$(uname -s)" in
  Darwin) cache="$HOME/Library/Caches/org.Zellij-Contributors.Zellij" ;;
  *)      cache="${XDG_CACHE_HOME:-$HOME/.cache}/zellij" ;;
esac
# zellij keys the grant by the path in the alias (~ expanded, symlinks not resolved): grant both the
# stable link path and the checkout's own path
mkdir -p "$cache"
for wasm in "$link/claude-tabs.wasm" "$here/claude-tabs.wasm"; do
  if ! grep -qsF "\"$wasm\"" "$cache/permissions.kdl"; then
    printf '"%s" {\n    ReadApplicationState\n    ChangeApplicationState\n    RunCommands\n    ReadCliPipes\n}\n' "$wasm" >>"$cache/permissions.kdl"
    echo "  claude-tabs: permissions pre-granted for $wasm in $cache/permissions.kdl"
  fi
done

[ "$hooks" = 1 ] || exit 0
[ -n "$targets" ] || { echo "  claude-tabs: no Claude config dir found, hooks not added" >&2; exit 0; }
hook="$here/claude-hook.sh"
case "$hook" in *' '*) hook="\"$hook\"" ;; esac
command -v jq >/dev/null 2>&1 || { echo "  claude-tabs: jq not found, add the hooks by hand (see README.md)" >&2; exit 1; }

# add_hooks FILE: merge the hooks into one settings file (0 = done or already present, 1 = failed)
add_hooks() {
  settings=$1
  sdir=$(dirname "$settings")
  if [ ! -d "$sdir" ]; then
    echo "  claude-tabs: $sdir does not exist, skipped $settings"
    return 0
  fi
  created=0
  if [ ! -s "$settings" ]; then printf '{}\n' >"$settings" || return 1; created=1; fi
  tmp="$settings.tmp.$$"
  if jq --arg h "$hook" '
      def repoint: if ((.command // "") | test("claude-hook\\.sh"))
                   then .command |= sub("^(\"[^\"]*claude-hook\\.sh\"|[^ ]*claude-hook\\.sh)"; $h) else . end;
      def add(ev; arg):
        if ((.hooks[ev] // []) | map(.hooks[]?.command // "") | any(test("claude-hook\\.sh"))) then .
        else .hooks[ev] = ((.hooks[ev] // []) + [{"hooks":[{"type":"command","command":($h + " " + arg)}]}]) end;
      .hooks = (.hooks // {})
      | .hooks |= with_entries(.value |= map(if (.hooks | type) == "array" then .hooks |= map(repoint) else . end))
      | add("SessionStart"; "start") | add("UserPromptSubmit"; "working") | add("Stop"; "done")
      | add("SessionEnd"; "end") | add("SubagentStart"; "substart") | add("SubagentStop"; "substop")
      | add("PreToolUse"; "tool")' "$settings" >"$tmp" && [ -s "$tmp" ]; then
    if cmp -s "$tmp" "$settings"; then
      rm -f "$tmp"; echo "  claude-tabs: hooks already present in $settings"
    elif [ "$created" = 1 ]; then
      cat "$tmp" >"$settings" && rm -f "$tmp" && changed=1 && echo "  claude-tabs: hooks set in new $settings"
    else
      cp "$settings" "$settings.bak-claude-tabs" && cat "$tmp" >"$settings" && rm -f "$tmp" && changed=1 \
        && echo "  claude-tabs: hooks set in $settings (backup: $settings.bak-claude-tabs)"
    fi
  else
    rm -f "$tmp"; echo "  claude-tabs: could not update $settings (invalid JSON?)" >&2; return 1
  fi
}

rc=0; seen=""; changed=0
while IFS= read -r f; do
  [ -n "$f" ] || continue
  case "$nl$seen" in *"$nl$f$nl"*) continue ;; esac   # same path given twice
  seen="$seen$f$nl"
  add_hooks "$f" || rc=1
done <<EOF
$targets
EOF
[ "$changed" = 1 ] && echo "  claude-tabs: restart running claude sessions to pick up new hooks"
exit $rc
