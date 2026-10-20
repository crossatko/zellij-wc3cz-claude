#!/bin/sh
# Build the plugin and hot-reload it in running zellij sessions (no restart, no permission prompt,
# hook state files in /tmp/zellij-<uid>/zellij-claude are untouched).
#   dev.sh                 build + reload in $ZELLIJ_SESSION_NAME
#   dev.sh -s NAME         ... in session NAME
#   dev.sh --all           ... in every running (not exited) session
#   dev.sh --watch [...]   rebuild + reload whenever src/, Cargo.toml changes (inotifywait, fswatch, entr or polling)
# Reload goes through the `claude-tabs` plugin alias from config.kdl, which reloads every instance
# (one per tab) of that alias. Sessions started before the alias existed need a restart once.
set -u
cd "$(dirname "$0")" || exit 1
mode=cur; session=${ZELLIJ_SESSION_NAME:-}; watch=0
while [ $# -gt 0 ]; do
  case "$1" in
    --all) mode=all ;;
    -s) shift; session=$1; mode=cur ;;
    --watch) watch=1 ;;
    -h|--help) sed -n '2,9p' "$0"; exit 0 ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
  shift
done

reload() {
  if [ "$mode" = all ]; then
    # running sessions only (list-sessions also lists exited, resurrectable ones)
    sessions=$(zellij list-sessions --no-formatting 2>/dev/null | grep -v EXITED | awk '{print $1}')
  else
    sessions=$session
  fi
  [ -n "$sessions" ] || { echo "no session to reload (not inside zellij? use -s NAME or --all)" >&2; return 1; }
  for s in $sessions; do
    if zellij --session "$s" action start-or-reload-plugin claude-tabs >/dev/null 2>&1; then
      echo "reloaded in $s"
    else
      echo "reload failed in $s" >&2
    fi
  done
}

once() {
  cargo build --release --target wasm32-wasip1 -q || { echo "build failed" >&2; return 1; }
  # the layout loads the tracked prebuilt copy (macOS users need no Rust toolchain)
  cp target/wasm32-wasip1/release/claude-tabs.wasm claude-tabs.wasm || return 1
  reload
}

once
[ "$watch" = 1 ] || exit $?

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
