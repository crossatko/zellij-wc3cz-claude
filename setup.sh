#!/bin/sh
# claude-tabs setup helper, Linux and macOS (POSIX sh, BSD-safe). OPTIONAL: the package works without it
# (README "Quick start": install the Claude Code plugin, add the bar to a zellij layout). Without options it
# only prints the status and what you could do; it changes NOTHING. Each change needs its own option:
#   setup.sh                       status + instructions (dry run, writes nothing)
#   setup.sh --font                Linux: install the WC3 icon font + fontconfig rules (fontconfig/install.sh;
#                                  2 files owned by this package, --font-uninstall removes them)
#   setup.sh --permissions         pre-grant the zellij plugin permissions for this checkout's wasm path(s) in
#                                  zellij's permissions.kdl (else zellij asks once: press y)
#   setup.sh --build               rebuild claude-tabs.wasm (needs Rust + the wasm32-wasip1 target)
#   setup.sh --migrate-to-plugin   remove the claude-hook.sh entries an older setup.sh wrote into Claude
#                                  settings files (now the Claude Code plugin provides the hooks); backup
#                                  <file>.bak-claude-tabs, other hooks/keys untouched, idempotent
#   setup.sh --link                LEGACY: link <zellij config dir>/plugins/claude-tabs -> this checkout
#   setup.sh --legacy-settings-hooks  LEGACY: write the hooks into Claude settings files instead of using the
#                                  plugin (use one route, not both: the script drops duplicate events)
# Settings file targets for --migrate-to-plugin / --legacy-settings-hooks / the status check:
#   --settings FILE (repeatable) and/or --all-claude-dirs (every ~/.claude* directory); default
#   $CLAUDE_SETTINGS, else $CLAUDE_CONFIG_DIR/settings.json, else ~/.claude/settings.json.
# Compatibility: --hooks = --legacy-settings-hooks (deprecated); --no-hooks is accepted and does nothing.
here=$(cd "$(dirname "$0")" && pwd -P)
here_l=$(cd "$(dirname "$0")" && pwd -L)
plugin_name=zellij-wc3cz
do_font=0; do_font_un=0; do_perm=0; do_build=0; do_link=0; do_hooks=0; do_migrate=0; all_dirs=0
targets=""      # newline-separated settings files
nl='
'
add_target() { targets="$targets$1$nl"; }
while [ $# -gt 0 ]; do
  case "$1" in
    --font) do_font=1 ;;
    --font-uninstall) do_font_un=1 ;;
    --permissions) do_perm=1 ;;
    --build) do_build=1 ;;
    --link) do_link=1 ;;
    --legacy-settings-hooks) do_hooks=1 ;;
    --hooks) do_hooks=1; echo "  claude-tabs: --hooks is deprecated, it means --legacy-settings-hooks (prefer the Claude Code plugin)" >&2 ;;
    --no-hooks) ;;
    --migrate-to-plugin) do_migrate=1 ;;
    --settings) [ $# -ge 2 ] || { echo "--settings needs a path" >&2; exit 2; }; shift; add_target "$1" ;;
    --settings=*) add_target "${1#--settings=}" ;;
    --all-claude-dirs) all_dirs=1 ;;
    -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
    *) echo "unknown option $1 (see --help)" >&2; exit 2 ;;
  esac
  shift
done
if [ "$do_hooks" = 1 ] && [ "$do_migrate" = 1 ]; then
  echo "--legacy-settings-hooks and --migrate-to-plugin contradict each other" >&2; exit 2
fi
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
any=$((do_font + do_font_un + do_perm + do_build + do_link + do_hooks + do_migrate))

zcfg=${ZELLIJ_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/zellij}
zcfg=${zcfg%/}
link="$zcfg/plugins/claude-tabs"
# zellij's cache dir: ProjectDirs("org", "Zellij Contributors", "Zellij").cache_dir()
case "$(uname -s)" in
  Darwin) zcache="$HOME/Library/Caches/org.Zellij-Contributors.Zellij" ;;
  *)      zcache="${XDG_CACHE_HOME:-$HOME/.cache}/zellij" ;;
esac
perms="$zcache/permissions.kdl"
# wasm paths zellij may know this checkout by (grants are keyed by the exact, ~-expanded path, symlinks
# not resolved): the physical path, the path this script was called by, the legacy link
wasm_paths() {
  printf '%s\n' "$here/claude-tabs.wasm"
  [ "$here_l" != "$here" ] && printf '%s\n' "$here_l/claude-tabs.wasm"
  if [ -L "$link" ] && [ "$(cd "$link" 2>/dev/null && pwd -P)" = "$here" ]; then printf '%s\n' "$link/claude-tabs.wasm"; fi
}
# our handlers: <path>/claude-hook.sh <event arg> (another tool's script of the same name with other args is
# not ours)
hook_re='claude-hook\.sh"? +(start|working|done|end|substart|substop|tool|ask|notify|clear) *$'

# settings_has_hooks FILE: the file has hook commands calling a claude-hook.sh (written by an older setup.sh)
settings_has_hooks() {
  [ -s "$1" ] && command -v jq >/dev/null 2>&1 \
    && jq -e --arg re "$hook_re" '[.hooks // {} | .[]? | .[]? | .hooks[]? | .command // "" | test($re)] | any' "$1" >/dev/null 2>&1
}
# plugin_installed DIR: the Claude Code plugin is installed in that Claude config dir
plugin_installed() {
  f="$1/plugins/installed_plugins.json"
  [ -s "$f" ] && command -v jq >/dev/null 2>&1 \
    && jq -e --arg p "$plugin_name@" '.plugins // {} | keys | any(startswith($p))' "$f" >/dev/null 2>&1
}

status() {
  echo "claude-tabs status (package: $here)"
  echo "  Claude Code hooks:"
  while IFS= read -r s; do
    [ -n "$s" ] || continue
    d=$(dirname "$s")
    p=no; plugin_installed "$d" && p=yes
    command -v jq >/dev/null 2>&1 || p="unknown (needs jq)"
    h=no; settings_has_hooks "$s" && h=yes
    command -v jq >/dev/null 2>&1 || h="unknown (needs jq)"
    echo "    $d: plugin $plugin_name installed: $p; old claude-hook.sh hooks in $(basename "$s"): $h"
    if [ "$p" = yes ] && [ "$h" = yes ]; then
      echo "      DOUBLE REGISTRATION: both routes run (the script drops the duplicate event, but use one). Fix: $0 --migrate-to-plugin --settings $s"
    elif [ "$p" = no ]; then
      [ "$h" = yes ] && echo "      legacy settings hooks in use; to switch: install the plugin, then $0 --migrate-to-plugin --settings $s"
      [ "$h" = no ] && echo "      no hooks yet; install the plugin:"
      echo "        claude plugin marketplace add crossatko/zellij-wc3cz-claude   (or a local checkout path)"
      echo "        claude plugin install $plugin_name@$plugin_name"
    fi
  done <<EOF
$targets
EOF
  echo "  zellij layout: put this pane into a layout (see layouts/claude.kdl):"
  echo "      pane size=1 borderless=true {"
  echo "          plugin location=\"file:$here/claude-tabs.wasm\""
  echo "      }"
  g=$(wasm_paths | while IFS= read -r w; do grep -qsF "\"$w\"" "$perms" && printf ' %s' "$w"; done)
  if [ -n "$g" ]; then echo "  zellij permissions pre-granted for:$g"
  else echo "  zellij permissions: not pre-granted (zellij asks once, press y; or: $0 --permissions)"; fi
  if command -v fc-list >/dev/null 2>&1 && [ -n "$(fc-list ':family=WC3 Icons' 2>/dev/null)" ]; then
    echo "  WC3 icon font: installed (colour character icons)"
  else
    case "$(uname -s)" in
      Darwin) echo "  WC3 icon font: not available on macOS (initials badges)" ;;
      *) echo "  WC3 icon font: not installed (initials badges; colour icons: $0 --font)" ;;
    esac
  fi
  if [ -L "$link" ] || [ -e "$link" ]; then echo "  legacy link $link exists (no longer needed)"; fi
}

if [ "$any" = 0 ]; then
  status
  echo "Nothing was changed. Options: $0 --help"
  exit 0
fi

rc=0
if [ "$do_font_un" = 1 ]; then sh "$here/fontconfig/install.sh" --uninstall || rc=1; fi
if [ "$do_font" = 1 ]; then sh "$here/fontconfig/install.sh" || rc=1; fi

if [ "$do_build" = 1 ]; then
  if command -v cargo >/dev/null 2>&1 && rustc --print target-libdir --target wasm32-wasip1 >/dev/null 2>&1 \
     && [ -d "$(rustc --print target-libdir --target wasm32-wasip1 2>/dev/null)" ]; then
    if (cd "$here" && cargo build --release --target wasm32-wasip1 -q); then
      cp "$here/target/wasm32-wasip1/release/claude-tabs.wasm" "$here/claude-tabs.wasm"
      echo "  claude-tabs: rebuilt claude-tabs.wasm"
    else
      echo "  claude-tabs: build failed, keeping the prebuilt claude-tabs.wasm" >&2; rc=1
    fi
  else
    echo "  claude-tabs: no wasm32-wasip1 toolchain (Arch: sudo pacman -S rust rust-wasm; macOS: brew install rustup &&"
    echo "    rustup-init -y && rustup target add wasm32-wasip1); the prebuilt claude-tabs.wasm is used"
  fi
fi

if [ "$do_link" = 1 ]; then
  if [ -e "$link" ] || [ -L "$link" ]; then
    if [ "$(cd "$link" 2>/dev/null && pwd -P)" != "$here" ]; then
      echo "  claude-tabs: $link exists and is not this checkout; left alone" >&2
    else
      echo "  claude-tabs: $link already points here"
    fi
  else
    mkdir -p "$zcfg/plugins" && ln -s "$here" "$link" && echo "  claude-tabs: linked $link -> $here"
  fi
fi

if [ "$do_perm" = 1 ]; then
  # append the missing grants to a copy of permissions.kdl in the same directory and rename it over the
  # original (atomic; existing content kept byte for byte, a missing final newline is added first)
  mkdir -p "$zcache" || rc=1
  ptmp=$(mktemp "$zcache/.permissions.kdl.XXXXXX") || { echo "  claude-tabs: cannot write in $zcache" >&2; ptmp=; rc=1; }
  if [ -n "$ptmp" ]; then
    [ -f "$perms" ] && cat "$perms" >"$ptmp"
    if [ -s "$ptmp" ] && [ -n "$(tail -c 1 "$ptmp")" ]; then printf '\n' >>"$ptmp"; fi
    added=""
    wasm_paths >"$ptmp.list"
    while IFS= read -r w; do
      if grep -qsF "\"$w\"" "$perms"; then
        echo "  claude-tabs: permissions already granted for $w"
      else
        printf '"%s" {\n    ReadApplicationState\n    ChangeApplicationState\n    RunCommands\n    ReadCliPipes\n}\n' "$w" >>"$ptmp"
        added="$added $w"
      fi
    done <"$ptmp.list"
    rm -f "$ptmp.list"
    if [ -n "$added" ]; then
      if [ -f "$perms" ]; then chmod --reference="$perms" "$ptmp" 2>/dev/null || chmod 644 "$ptmp"; else chmod 644 "$ptmp"; fi
      if mv -f "$ptmp" "$perms"; then echo "  claude-tabs: permissions pre-granted for$added in $perms"; else rm -f "$ptmp"; rc=1; fi
    else
      rm -f "$ptmp"
    fi
  fi
fi

[ "$do_hooks" = 1 ] || [ "$do_migrate" = 1 ] || exit $rc
command -v jq >/dev/null 2>&1 || { echo "  claude-tabs: jq not found, edit the settings file(s) by hand (see README.md)" >&2; exit 1; }
hook="$here/claude-hook.sh"
case "$hook" in *' '*) hook="\"$hook\"" ;; esac

# edit_settings FILE: apply the hook change (legacy add or migration removal) to one settings file
# (0 = done or nothing to do, 1 = failed)
edit_settings() {
  settings=$1
  sdir=$(dirname "$settings")
  if [ ! -d "$sdir" ]; then
    echo "  claude-tabs: $sdir does not exist, skipped $settings"
    return 0
  fi
  created=0
  if [ ! -s "$settings" ]; then
    if [ "$do_migrate" = 1 ]; then echo "  claude-tabs: $settings missing or empty, nothing to migrate"; return 0; fi
    printf '{}\n' >"$settings" || return 1; created=1
  fi
  tmp="$settings.tmp.$$"
  if [ "$do_migrate" = 1 ]; then
    # drop only handlers calling a claude-hook.sh; then matcher groups left without handlers, events left
    # without groups, and an emptied "hooks" object (only when we emptied it)
    filter='
      if (.hooks | type) != "object" then .
      else
        (.hooks | length) as $before
        | .hooks |= with_entries(
            .value |= (if type == "array" then
                map(if (.hooks | type) == "array"
                    then (.hooks |= map(select(((.command // "") | test($re)) | not)))
                    else . end)
                | map(select((.hooks | type) != "array" or (.hooks | length) > 0))
              else . end))
        | .hooks |= with_entries(select((.value | type) != "array" or (.value | length) > 0))
        | if ($before > 0 and (.hooks | length) == 0) then del(.hooks) else . end
      end'
  else
    filter='
      def repoint: if ((.command // "") | test($re))
                   then .command |= sub("^(\"[^\"]*claude-hook\\.sh\"|[^ ]*claude-hook\\.sh)"; $h) else . end;
      def add(ev; arg):
        if ((.hooks[ev] // []) | map(.hooks[]?.command // "") | any(test($re))) then .
        else .hooks[ev] = ((.hooks[ev] // []) + [{"hooks":[{"type":"command","command":($h + " " + arg)}]}]) end;
      .hooks = (.hooks // {})
      | .hooks |= with_entries(.value |= map(if (.hooks | type) == "array" then .hooks |= map(repoint) else . end))
      | add("SessionStart"; "start") | add("UserPromptSubmit"; "working") | add("Stop"; "done")
      | add("SessionEnd"; "end") | add("SubagentStart"; "substart") | add("SubagentStop"; "substop")
      | add("PreToolUse"; "tool") | add("PermissionRequest"; "ask") | add("Notification"; "notify")
      | add("PostToolUse"; "clear") | add("PostToolUseFailure"; "clear")'
  fi
  if jq --arg h "$hook" --arg re "$hook_re" "$filter" "$settings" >"$tmp" && [ -s "$tmp" ]; then
    if jq -e . "$tmp" >/dev/null 2>&1 && [ "$(jq -S . "$tmp")" = "$(jq -S . "$settings")" ]; then
      rm -f "$tmp"
      if [ "$do_migrate" = 1 ]; then echo "  claude-tabs: no old claude-hook.sh hooks in $settings"
      else echo "  claude-tabs: hooks already present in $settings"; fi
    elif [ "$created" = 1 ]; then
      cat "$tmp" >"$settings" && rm -f "$tmp" && changed=1 && echo "  claude-tabs: hooks set in new $settings"
    else
      cp "$settings" "$settings.bak-claude-tabs" && cat "$tmp" >"$settings" && rm -f "$tmp" && changed=1 \
        && if [ "$do_migrate" = 1 ]; then
             echo "  claude-tabs: removed the old claude-hook.sh hooks from $settings (backup: $settings.bak-claude-tabs)"
           else
             echo "  claude-tabs: hooks set in $settings (backup: $settings.bak-claude-tabs)"
           fi
    fi
    if [ "$do_hooks" = 1 ] && plugin_installed "$sdir"; then
      echo "  claude-tabs: WARNING the $plugin_name plugin is installed in $sdir too: both hook routes now run (the duplicate event is dropped by the script, but use one route)" >&2
    fi
    if [ "$do_migrate" = 1 ] && ! plugin_installed "$sdir"; then
      echo "  claude-tabs: note: the $plugin_name plugin is not installed in $sdir; install it or there are no hooks"
    fi
  else
    rm -f "$tmp"; echo "  claude-tabs: could not update $settings (invalid JSON?)" >&2; return 1
  fi
}

seen=""; changed=0
while IFS= read -r f; do
  [ -n "$f" ] || continue
  case "$nl$seen" in *"$nl$f$nl"*) continue ;; esac   # same path given twice
  seen="$seen$f$nl"
  edit_settings "$f" || rc=1
done <<EOF
$targets
EOF
[ "$changed" = 1 ] && echo "  claude-tabs: restart running claude sessions to pick up the change"
exit $rc
