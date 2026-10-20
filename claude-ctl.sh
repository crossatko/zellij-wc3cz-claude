#!/bin/sh
# Control the claude-tabs plugin from a shell (runs inside a zellij pane).
#   claude-ctl.sh ding [character]   play a voice line now (default: the active tab's character)
#   claude-ctl.sh character <name>   set the character of the active tab, e.g. character "Arthas Menethill"
#   claude-ctl.sh race <race>        give the active tab a character of that race
#                                    (human|orc|undead|nightelf|naga|demon|neutral)
#   claude-ctl.sh reshuffle          forget all assignments and hand them out again (unique first)
# `zellij pipe` to a tab-bar plugin blocks until every per-tab instance answers, so it is capped with a timeout.
# this script's own directory, symlinks resolved (siblings and sounds/ live there)
self=$0
case $self in */*) ;; *) if [ -f "$self" ]; then self=./$self; else self=$(command -v "$self") || self=./$0; fi ;; esac
while [ -h "$self" ]; do
  _l=$(readlink "$self") || break
  case $_l in /*) self=$_l ;; *) self=${self%/*}/$_l ;; esac
done
here=$(cd -P "${self%/*}/" 2>/dev/null && pwd -P) || here=$(dirname "$0")
. "$here/claude-lib.sh"
cmd=${1:-}; arg=${2:-active}
usage() { echo "usage: $0 ding [character] | character <name> | race <race> | reshuffle" >&2; exit 2; }
case "$cmd" in
  ding) name=claude_ding ;;
  character) [ -n "${2:-}" ] || usage; name=claude_character ;;
  race) [ -n "${2:-}" ] || usage; name=claude_race ;;
  reshuffle) name=claude_reshuffle; arg=now ;;
  *) usage ;;
esac
printf '%s' "$arg" | run_timeout 2 zellij pipe --name "$name" >/dev/null 2>&1
exit 0
