#!/bin/sh
# Is the terminal window that shows zellij session SESSION the OS-focused window?
#   os-focus.sh [SESSION]      (default $ZELLIJ_SESSION_NAME)  -> prints focused | unfocused | unknown
# POSIX sh, bash 3.2 / BSD-safe; every external command is capped (claude-lib.sh run_timeout).
# zellij-tile 0.45 gives plugins no terminal/OS focus event, hence this helper (used by claude-hook.sh on
# Stop and polled by the plugin while a finished active tab waits for the user to come back).
#
# 1. Focused window (pid + title), first backend that answers:
#      macOS: lsappinfo (frontmost app pid, no title, no permission needed); osascript System Events
#             as a fallback (may ask once for Automation permission)
#      Hyprland: hyprctl activewindow -j     Sway: swaymsg -t get_tree (needs jq)
#      KDE Wayland: kdotool                  X11 (incl. i3, Xfce, KDE X11, ...): xdotool, else xprop
#      GNOME Wayland / unknown compositor: no generic API -> unknown
# 2. Decision:
#      title "SESSION | ..." (zellij titles the host terminal "<session> | <pane title>") -> focused
#      else the focused pid is an ancestor of a zellij client attached to SESSION (Linux: client found
#      through `ss -xp` socket peers of the session socket; elsewhere: every zellij client process),
#      unless the title names another running zellij session -> focused
#      else -> unfocused;  no focused window info -> unknown
# Limits: several windows of one terminal process (kitty, wezterm, Terminal.app, iTerm2) share a pid:
# without a usable title (macOS) another window of the same terminal counts as focused. Terminals in a
# pid namespace (flatpak) never match by pid.
# this script's own directory, symlinks resolved (siblings and sounds/ live there)
self=$0
case $self in */*) ;; *) if [ -f "$self" ]; then self=./$self; else self=$(command -v "$self") || self=./$0; fi ;; esac
while [ -h "$self" ]; do
  _l=$(readlink "$self") || break
  case $_l in /*) self=$_l ;; *) self=${self%/*}/$_l ;; esac
done
here=$(cd -P "${self%/*}/" 2>/dev/null && pwd -P) || here=$(dirname "$0")
. "$here/claude-lib.sh"
S=${1:-${ZELLIJ_SESSION_NAME:-}}
T=1   # seconds per external command

fpid=""; ftitle=""

json_field() {   # $1 key; reads pretty JSON on stdin, prints the first scalar value of "key"
  sed -n -e "s/^[[:space:]]*\"$1\"[[:space:]]*:[[:space:]]*\"\(.*\)\",\{0,1\}[[:space:]]*\$/\1/p" \
         -e "s/^[[:space:]]*\"$1\"[[:space:]]*:[[:space:]]*\([^\",]*\),\{0,1\}[[:space:]]*\$/\1/p" | head -n 1
}

focus_darwin() {
  if command -v lsappinfo >/dev/null 2>&1; then
    asn=$(run_timeout $T lsappinfo front 2>/dev/null)
    [ -n "$asn" ] && fpid=$(run_timeout $T lsappinfo info -only pid "$asn" 2>/dev/null | sed -n 's/.*"pid"=\([0-9][0-9]*\).*/\1/p')
  fi
  if [ -z "$fpid" ] && command -v osascript >/dev/null 2>&1; then
    fpid=$(run_timeout $T osascript -e 'tell application "System Events" to unix id of first application process whose frontmost is true' 2>/dev/null)
  fi
}

focus_hyprland() {
  j=$(run_timeout $T hyprctl activewindow -j 2>/dev/null) || return
  fpid=$(printf '%s\n' "$j" | json_field pid)
  ftitle=$(printf '%s\n' "$j" | json_field title)
}

focus_sway() {
  command -v jq >/dev/null 2>&1 || return
  l=$(run_timeout $T swaymsg -t get_tree 2>/dev/null \
      | jq -r '[.. | objects | select(.focused? == true)][0] | "\(.pid // "")\t\(.name // "")"' 2>/dev/null)
  fpid=${l%%"	"*}; ftitle=${l#*"	"}
}

focus_kdotool() {
  w=$(run_timeout $T kdotool getactivewindow 2>/dev/null) || return
  [ -n "$w" ] || return
  fpid=$(run_timeout $T kdotool getwindowpid "$w" 2>/dev/null)
  ftitle=$(run_timeout $T kdotool getwindowname "$w" 2>/dev/null)
}

focus_x11() {
  if command -v xdotool >/dev/null 2>&1; then
    w=$(run_timeout $T xdotool getactivewindow 2>/dev/null)
    if [ -n "$w" ]; then
      fpid=$(run_timeout $T xdotool getwindowpid "$w" 2>/dev/null)
      ftitle=$(run_timeout $T xdotool getwindowname "$w" 2>/dev/null)
      return
    fi
  fi
  command -v xprop >/dev/null 2>&1 || return
  w=$(run_timeout $T xprop -root _NET_ACTIVE_WINDOW 2>/dev/null | sed -n 's/.*# \(0x[0-9a-fA-F]*\).*/\1/p')
  case "$w" in ''|0x0) return ;; esac
  p=$(run_timeout $T xprop -id "$w" _NET_WM_PID _NET_WM_NAME 2>/dev/null)
  fpid=$(printf '%s\n' "$p" | sed -n 's/^_NET_WM_PID(CARDINAL) = \([0-9]*\).*/\1/p')
  ftitle=$(printf '%s\n' "$p" | sed -n 's/^_NET_WM_NAME([^)]*) = "\(.*\)"$/\1/p')
}

if [ "$(uname -s 2>/dev/null)" = Darwin ]; then
  focus_darwin
elif [ -n "${HYPRLAND_INSTANCE_SIGNATURE:-}" ] && command -v hyprctl >/dev/null 2>&1; then
  focus_hyprland
elif [ -n "${SWAYSOCK:-}" ] && command -v swaymsg >/dev/null 2>&1; then
  focus_sway
elif [ -n "${WAYLAND_DISPLAY:-}" ]; then
  # X11 tools only see Xwayland windows here: never trust them on Wayland
  case "${XDG_CURRENT_DESKTOP:-}" in
    *KDE*) command -v kdotool >/dev/null 2>&1 && focus_kdotool ;;
  esac
elif [ -n "${DISPLAY:-}" ]; then
  focus_x11
fi

case "$fpid" in ''|*[!0-9]*|0) echo unknown; exit 0 ;; esac

# ss_clients NAME: pids of the clients attached to session NAME (Linux; empty without ss)
ss_clients() {
  command -v ss >/dev/null 2>&1 || return 0
  # server side of each client connection: local path .../<session>, peer inode = the client's socket
  run_timeout $T ss -xpH 2>/dev/null | awk -v s="/$1" '
    { n = length($5); ls = length(s) }
    n >= ls && substr($5, n - ls + 1) == s && $8 != "0" && $8 != "*" { peer[$8] = 1; next }
    { line[NR] = $0; ino[NR] = $6 }
    END { for (i in line) if (ino[i] in peer) { t = line[i]
            while (match(t, /pid=[0-9]+/)) { print substr(t, RSTART + 4, RLENGTH - 4); t = substr(t, RSTART + RLENGTH) } } }' | sort -u
}
# under_fpid PID...: success when the focused window's process is an ancestor of (or is) one of the pids
under_fpid() {
  for c in "$@"; do
    p=$c; i=0
    while [ -n "$p" ] && [ "$p" -gt 1 ] 2>/dev/null && [ $i -lt 30 ]; do
      [ "$p" = "$fpid" ] && return 0
      p=$(ps -o ppid= -p "$p" 2>/dev/null | tr -d ' ')
      i=$((i + 1))
    done
  done
  return 1
}
# 1) the title zellij gives the host terminal. It is only rewritten when a pane title changes, so it can be
# stale; a session with no attached client at all (ss) is not being looked at whatever the title says.
clients=""
[ -n "$S" ] && clients=$(ss_clients "$S")
if [ -n "$S" ]; then
  case "$ftitle" in
    "$S | "*|"$S")
      if [ -n "$clients" ] || ! command -v ss >/dev/null 2>&1; then echo focused; else echo unfocused; fi
      exit 0 ;;
  esac
fi

# 2) pid ancestry of the zellij clients of this session
if [ -z "$clients" ] && ! command -v ss >/dev/null 2>&1; then
  # fallback (macOS, no ss): every zellij client process (not the server, not CLI subcommands)
  clients=$(ps -axo pid=,args= 2>/dev/null | awk '
    { a = $2; sub(/.*\//, "", a) }
    a == "zellij" && $0 !~ / --server / && $0 !~ / (pipe|action|run|edit|list-sessions|ls|kill-session|delete-session|setup|plugin|web)( |$)/ { print $1 }')
fi
if under_fpid $clients; then
  # same terminal process: another window of it may be focused, so a title naming another session counts
  # as "elsewhere" -- unless that title is stale. The terminal title is only rewritten when a pane title
  # changes, so after switching session inside the client (session manager) it still names the OLD
  # session: then no client of that session lives in this terminal any more.
  other=${ftitle%% | *}
  if [ -n "$ftitle" ] && [ "$other" != "$ftitle" ] && [ "$other" != "$S" ] \
     && run_timeout $T zellij list-sessions --short --no-formatting 2>/dev/null | grep -qxF "$other"; then
    # without ss (macOS) staleness cannot be told: trust the title
    if ! command -v ss >/dev/null 2>&1 || under_fpid $(ss_clients "$other"); then echo unfocused; exit 0; fi
  fi
  echo focused; exit 0
fi
echo unfocused
exit 0
