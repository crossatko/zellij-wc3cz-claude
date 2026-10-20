#!/bin/sh
# Control the claude-tabs plugin from a shell (runs inside a zellij pane).
#   claude-ctl.sh ding [race]   play a race phrase now (default: the active tab's race)
#   claude-ctl.sh race <race>   set the race of the active tab (human|orc|undead|nightelf|naga|demon|neutral)
#   claude-ctl.sh reshuffle     forget all race assignments and hand them out again (unique first)
# `zellij pipe` to a tab-bar plugin blocks until every per-tab instance answers, so it is capped with a timeout.
. "$(dirname "$0")/claude-lib.sh"
cmd=$1; arg=${2:-active}
case "$cmd" in
  ding) name=claude_ding ;;
  race) name=claude_race ;;
  reshuffle) name=claude_reshuffle; arg=now ;;
  *) echo "usage: $0 ding [race] | race <race> | reshuffle" >&2; exit 2 ;;
esac
printf '%s' "$arg" | run_timeout 2 zellij pipe --name "$name" >/dev/null 2>&1
exit 0
