#!/bin/sh
# Build the plugin and hot-reload it in running zellij sessions (no restart, no permission prompt,
# hook state files in /tmp/zellij-<uid>/zellij-claude are untouched).
#   dev.sh                 build + reload in $ZELLIJ_SESSION_NAME
#   dev.sh -s NAME         ... in session NAME
#   dev.sh --all           ... in every running (not exited) session
#   dev.sh --watch [...]   rebuild + reload whenever src/, Cargo.toml changes (inotifywait, fswatch, entr or polling)
#   dev.sh -p URL          reload the plugin loaded as exactly URL
#   dev.sh -a [NAME]       reload through the plugin alias NAME (default claude-tabs)
# Without -p/-a each session is inspected (`zellij action list-panes -a`, COMMAND column = the location string
# the plugin pane was loaded with): alias `claude-tabs` if a plugin pane was loaded through it, else
# file:<this checkout>/claude-tabs.wasm if a plugin pane has exactly that location, else the session is
# REFUSED (non-zero exit). `zellij action start-or-reload-plugin X` reloads every instance (one per tab) of
# the plugin loaded from exactly X, but when X matches nothing it LAUNCHES a new plugin pane in the focused
# tab, so dev.sh never calls it with a target that is not already present in the session. A layout that
# loads the plugin from another path or a symlink needs -p with that exact string (and a copy of the new
# wasm there). Alias options and layout options are NOT re-read by a reload.
set -u
cd "$(dirname "$0")" || exit 1
mode=cur; session=${ZELLIJ_SESSION_NAME:-}; watch=0
target=""; own="file:$PWD/claude-tabs.wasm"
while [ $# -gt 0 ]; do
  case "$1" in
    --all) mode=all ;;
    -s) shift; session=$1; mode=cur ;;
    --watch) watch=1 ;;
    -p) shift; [ $# -gt 0 ] || { echo "-p needs a URL" >&2; exit 2; }; target=$1 ;;
    -a) target=claude-tabs; case "${2:-}" in ""|-*) ;; *) shift; target=$1 ;; esac ;;
    -h|--help) sed -n '2,15p' "$0"; exit 0 ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
  shift
done

# does session $1 have a plugin pane whose location string is exactly $2? (list-panes columns are separated
# by two spaces: tab id, pos, tab name, pane id, type, title, command, ...)
has_plugin() {
  zellij --session "$1" action list-panes -a 2>/dev/null |
    awk -F'  ' -v t="$2" '$5 == "plugin" && $7 == t { f = 1 } END { exit !f }'
}

reload() {
  if [ "$mode" = all ]; then
    # running sessions only (list-sessions also lists exited, resurrectable ones)
    sessions=$(zellij list-sessions --no-formatting 2>/dev/null | grep -v EXITED | awk '{print $1}')
  else
    sessions=$session
  fi
  [ -n "$sessions" ] || { echo "no session to reload (not inside zellij? use -s NAME or --all)" >&2; return 1; }
  rc=0
  for s in $sessions; do
    if [ -n "$target" ]; then t=$target
    elif has_plugin "$s" claude-tabs; then t=claude-tabs
    elif has_plugin "$s" "$own"; then t=$own
    else t=
    fi
    if [ -z "$t" ] || ! has_plugin "$s" "$t"; then
      echo "refused in $s: no plugin pane loaded as '${t:-claude-tabs}' or '$own' (reloading an unknown location would open a new pane); use -p with the exact location of the layout" >&2
      rc=1; continue
    fi
    if zellij --session "$s" action start-or-reload-plugin "$t" >/dev/null 2>&1; then
      echo "reload requested in $s ($t)"
    else
      echo "reload failed in $s" >&2; rc=1
    fi
  done
  return $rc
}

once() {
  cargo build --release --target wasm32-wasip1 -q || { echo "build failed" >&2; return 1; }
  # the layout loads the tracked prebuilt copy (macOS users need no Rust toolchain)
  cp target/wasm32-wasip1/release/claude-tabs.wasm claude-tabs.wasm || return 1
  reload
}

once; rc=$?
[ "$watch" = 1 ] || exit $rc

echo "watching src/ and Cargo.toml (ctrl-c to stop)"
if command -v inotifywait >/dev/null 2>&1; then
  while inotifywait -qq -e close_write,moved_to,create -r src Cargo.toml; do sleep 0.2; once; done
elif command -v fswatch >/dev/null 2>&1; then   # macOS: brew install fswatch
  fswatch -o -l 0.3 src Cargo.toml | while read -r _; do once; done
elif command -v entr >/dev/null 2>&1; then
  while :; do find src Cargo.toml -type f | entr -d -p -s true && once; done
else
  # portable polling: GNU stat on Linux, BSD stat on macOS; cksum exists on both
  if stat -c %Y . >/dev/null 2>&1; then statcmd="stat -c %Y"; else statcmd="stat -f %m"; fi
  stamp() { find src Cargo.toml -type f -exec $statcmd {} + | cksum; }
  last=$(stamp)
  while sleep 1; do
    cur=$(stamp)
    [ "$cur" = "$last" ] || { last=$cur; once; }
  done
fi
