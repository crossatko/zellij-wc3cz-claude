#!/bin/sh
# Claude Code hook -> zellij claude-tabs plugin.
# Usage: claude-hook.sh start|working|done|end|substart|substop|tool   (hook JSON arrives on stdin)
# Writes the per-pane state the plugin reads, then pokes it. Silent no-op outside zellij.
[ -n "$ZELLIJ" ] && [ -n "$ZELLIJ_PANE_ID" ] && [ -n "$ZELLIJ_SESSION_NAME" ] || exit 0
. "$(dirname "$0")/claude-lib.sh" 2>/dev/null || exit 0
root=$(claude_state_root)
dir="$root/$ZELLIJ_SESSION_NAME"
f="$dir/$ZELLIJ_PANE_ID"
sub="$f.sub"
mkdir -p "$dir" 2>/dev/null || exit 0
write() { printf '%s %s%s\n' "$1" "$(now_ns)" "$2" >"$f.tmp" && mv -f "$f.tmp" "$f"; }
# debug: `touch <state root>/debug` logs every event (with stdin JSON) to <state root>/debug.log
if [ -e "$root/debug" ]; then
  in=$(cat); printf '%s %s pane=%s %s\n' "$(date +%T)" "$1" "$ZELLIJ_PANE_ID" "$in" >>"$root/debug.log"
  set -- "$1" "$in"
fi
# hook JSON from stdin (not for `tool`: it runs per tool call and must stay cheap)
if [ "$1" != tool ] && [ -z "$2" ]; then set -- "$1" "$(cat)"; fi
# remember where this session keeps its background agents' transcripts (read by the plugin, claude-agents.sh)
if [ "$1" != tool ] && [ "$1" != end ]; then
  tp=$(printf '%s' "$2" | sed -n 's/.*"transcript_path"[[:space:]]*:[[:space:]]*"\([^"]*\)\.jsonl".*/\1/p' | head -n 1)
  [ -n "$tp" ] && printf '%s/subagents' "$tp" >"$f.sess"
fi
# any event except the end of the session counts as activity (the plugin's `working_hold_s` safety net)
[ "$1" = end ] || : >"$f.ev"
case "$1" in
  tool)    # optional PreToolUse/PostToolUse hook: a turn that started without UserPromptSubmit (the main
           # agent woken by a finished subagent) is working
           cur=$(cut -d' ' -f1 "$f" 2>/dev/null)
           [ "$cur" = working ] || write working
           exit 0 ;;
  start)   rm -rf "$sub"; write idle ;;
  working) write working ;;
  done)    # background subagents may outlive the main turn: keep their markers, and tell the plugin that
           # this Stop is not a real finish while a (non-stale) subagent marker exists
           stale_min=$(( ${ZELLIJ_CLAUDE_STALE_S:-900} / 60 ))
           running=""
           [ -f "$f.sess" ] && running=$("$(dirname "$0")/claude-agents.sh" "$(cat "$f.sess")" 2>/dev/null | head -n 1)
           # " away" = the terminal window showing this session is not the OS-focused window (os-focus.sh;
           # unknown counts as not away); the plugin then also dings/blinks the active tab.
           # ZELLIJ_CLAUDE_OS_FOCUS=0 skips the check.
           away=""
           if [ "${ZELLIJ_CLAUDE_OS_FOCUS:-1}" != 0 ] \
              && [ "$(run_timeout 1 "$(dirname "$0")/os-focus.sh" "$ZELLIJ_SESSION_NAME" 2>/dev/null)" = unfocused ]; then
             away=" away"
           fi
           if [ -n "$running" ] || [ -n "$(find "$sub" -type f -mmin -"$stale_min" 2>/dev/null | head -n 1)" ]; then write done " sub$away"; else write done "$away"; fi ;;
  end)     rm -f "$f" "$f.ev" "$f.sess"; rm -rf "$sub" ;;
  substart|substop)
    [ -n "$2" ] || set -- "$1" "$(cat)"
    id=$(printf '%s' "$2" | sed -n 's/.*"agent_id"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1 | tr -c 'A-Za-z0-9_\n-' '_')
    [ -n "$id" ] || id="anon$$"
    if [ "$1" = substart ]; then
      mkdir -p "$sub" && : >"$sub/$id"
    else
      rm -f "$sub/$id"
    fi ;;
  *) exit 0 ;;
esac
# poke is best effort and must never delay or fail Claude
( run_timeout 3 zellij pipe --name claude_poke -- "" </dev/null >/dev/null 2>&1 & ) 2>/dev/null
exit 0
