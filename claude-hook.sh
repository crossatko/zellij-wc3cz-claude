#!/bin/sh
# Claude Code hook -> zellij claude-tabs plugin.
# Usage: claude-hook.sh start|working|done|end|substart|substop|tool|ask|notify|clear   (hook JSON arrives on stdin)
# ask (PermissionRequest) / notify (Notification) / tool (PreToolUse AskUserQuestion, ExitPlanMode) raise the
# attention overlay <pane>.attn ("<ns> [away] ag=<agent id or ->"); clear (PostToolUse, PostToolUseFailure),
# a later tool call, SubagentStop, UserPromptSubmit, Stop, SessionStart/End remove it.
# Writes the per-pane state the plugin reads, then pokes it. Silent no-op outside zellij.
# Also tells the plugin where this package lives (<state root>/pkgdir) and whether the WC3 Icons font is
# installed (<state root>/font, re-checked every 10 min): the wasm cannot find out either by itself.
[ -n "$ZELLIJ" ] && [ -n "$ZELLIJ_PANE_ID" ] && [ -n "$ZELLIJ_SESSION_NAME" ] || exit 0
# package dir = this script's own directory, symlinks resolved (Claude Code plugin cache, a checkout, a link)
self=$0
case $self in */*) ;; *) if [ -f "$self" ]; then self=./$self; else self=$(command -v "$self") || exit 0; fi ;; esac
while [ -h "$self" ]; do
  _l=$(readlink "$self") || break
  case $_l in /*) self=$_l ;; *) self=${self%/*}/$_l ;; esac
done
here=$(cd -P "${self%/*}/" 2>/dev/null && pwd -P) || exit 0
. "$here/claude-lib.sh" 2>/dev/null || exit 0
root=$(claude_state_root)
dir="$root/$ZELLIJ_SESSION_NAME"
f="$dir/$ZELLIJ_PANE_ID"
sub="$f.sub"
mkdir -p "$dir" 2>/dev/null || exit 0
# temp files are per process: a hook registered twice runs two copies of this script at the same time
write() { printf '%s %s%s\n' "$1" "$(now_ns)" "$2" >"$f.tmp.$$" && mv -f "$f.tmp.$$" "$f"; }
# put FILE CONTENT: atomic, and only when the content differs (most events change nothing)
put() {
  _c=""; [ -f "$1" ] && IFS= read -r _c <"$1"
  [ "$_c" = "$2" ] || { printf '%s\n' "$2" >"$1.$$" && mv -f "$1.$$" "$1"; }
}
put "$root/pkgdir" "$here"
# font flag: fc-list in the background (a cold fontconfig cache can take seconds); macOS = 0 (CoreText
# draws no CBDT colour bitmaps)
if [ -z "$(find "$root/font" -mmin -10 2>/dev/null)" ]; then
  [ -f "$root/font" ] && touch "$root/font" 2>/dev/null   # claim: no parallel re-checks
  ( v=0
    case "$(uname -s 2>/dev/null)" in
      Darwin) ;;
      *) command -v fc-list >/dev/null 2>&1 && run_timeout 10 fc-list 'WC3 Icons' family 2>/dev/null | grep -qi 'WC3 Icons' && v=1 ;;
    esac
    printf '%s\n' "$v" >"$root/font.$$" && mv -f "$root/font.$$" "$root/font" ) </dev/null >/dev/null 2>&1 &
fi
# debug: `touch <state root>/debug` logs every event (with stdin JSON) to <state root>/debug.log
if [ -e "$root/debug" ]; then
  in=$(cat); printf '%s %s pane=%s %s\n' "$(date +%T)" "$1" "$ZELLIJ_PANE_ID" "$in" >>"$root/debug.log"
  set -- "$1" "$in"
fi
# hook JSON from stdin
if [ -z "$2" ]; then set -- "$1" "$(cat)"; fi
json_str() { printf '%s' "$2" | sed -n 's/.*"'"$1"'"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1; }
# the terminal window of this session is not the OS-focused window (os-focus.sh; unknown = not away)
os_away() {
  [ "${ZELLIJ_CLAUDE_OS_FOCUS:-1}" != 0 ] \
    && [ "$(run_timeout 1 sh "$here/os-focus.sh" "$ZELLIJ_SESSION_NAME" 2>/dev/null)" = unfocused ] && printf ' away'
}
# attention: raise the overlay unless one is already pending (PermissionRequest, Notification and PreToolUse
# of one request must ding once)
attn_raise() {
  [ -e "$f.attn" ] && return 0
  [ -e "$f" ] || write idle
  ag=$(json_str agent_id "$2"); [ -n "$ag" ] || ag=-
  # link(2) never replaces: of two concurrent raises exactly one overlay (one ns, one ding) survives
  printf '%s%s ag=%s\n' "$(now_ns)" "$(os_away)" "$ag" >"$f.attn.$$" && { ln "$f.attn.$$" "$f.attn" 2>/dev/null; rm -f "$f.attn.$$"; }
  : >"$f.ev"
}
# attn_drop PAYLOAD: remove the overlay when the event comes from the agent that asked (or the asker is unknown)
attn_drop() {
  [ -e "$f.attn" ] || return 1
  want=$(sed -n 's/.* ag=\(.*\)$/\1/p' "$f.attn" 2>/dev/null | head -n 1)
  ag=$(json_str agent_id "$1"); [ -n "$ag" ] || ag=-
  if [ "$want" = - ] || [ "$want" = "$ag" ]; then rm -f "$f.attn"; return 0; fi
  return 1
}
# Notifications other than "needs an answer" (idle_prompt, auth_success, ...) are not activity either
if [ "$1" = notify ]; then
  case "$(json_str notification_type "$2")" in
    permission_prompt|elicitation_dialog|elicitation_url_dialog) ;;
    *) exit 0 ;;
  esac
fi
# The hooks may be registered twice (Claude Code plugin + an entry in settings.json from an older
# setup.sh): both copies get the same event with the same JSON at the same time. The first to create the
# marker <pane>.once.<event+payload checksum> handles it, the duplicate exits (a second Stop would ding
# twice). Any other event of the pane removes the marker (Claude Code finishes one event's hooks before
# the next event fires); tool/clear (every tool call, idempotent) only remove it, without a checksum.
case "$1" in
  tool|clear) for _o in "$f".once.*; do [ -d "$_o" ] && rmdir "$_o" 2>/dev/null; done ;;
  *) _k=$(printf '%s\n%s' "$1" "$2" | cksum | tr -c '0-9\n' '_')
     mkdir "$f.once.$_k" 2>/dev/null || exit 0
     for _o in "$f".once.*; do [ "$_o" = "$f.once.$_k" ] || { [ -d "$_o" ] && rmdir "$_o" 2>/dev/null; }; done ;;
esac
# remember where this session keeps its background agents' transcripts (read by the plugin, claude-agents.sh)
if [ "$1" != tool ] && [ "$1" != clear ] && [ "$1" != end ]; then
  tp=$(printf '%s' "$2" | sed -n 's/.*"transcript_path"[[:space:]]*:[[:space:]]*"\([^"]*\)\.jsonl".*/\1/p' | head -n 1)
  [ -n "$tp" ] && put "$f.sess" "$tp/subagents"
fi
# any event except the end of the session counts as activity (the plugin's `working_hold_s` safety net)
case "$1" in end|notify|ask|clear) ;; *) : >"$f.ev" ;; esac
case "$1" in
  tool)    # optional PreToolUse/PostToolUse hook: a turn that started without UserPromptSubmit (the main
           # agent woken by a finished subagent) is working
           # AskUserQuestion / ExitPlanMode wait for the owner: attention. Any other tool call of the asker
           # means the question was answered (a denied permission fires no hook of its own).
           case "$(json_str tool_name "$2")" in
             AskUserQuestion|ExitPlanMode) attn_raise "$@"; ( run_timeout 3 zellij pipe --name claude_poke -- "" </dev/null >/dev/null 2>&1 & ) 2>/dev/null; exit 0 ;;
           esac
           attn_drop "$2"
           cur=$(cut -d' ' -f1 "$f" 2>/dev/null)
           [ "$cur" = working ] || write working
           exit 0 ;;
  ask|notify) attn_raise "$@" ;;
  clear)   # PostToolUse / PostToolUseFailure: cheap no-op unless an attention request is pending
           [ -e "$f.attn" ] || exit 0
           attn_drop "$2" && : >"$f.ev" || exit 0 ;;
  start)   rm -rf "$sub"; rm -f "$f.attn"; write idle ;;
  working) rm -f "$f.attn"; write working ;;
  done)    # background subagents may outlive the main turn: keep their markers, and tell the plugin that
           # this Stop is not a real finish while a (non-stale) subagent marker exists
           stale_min=$(( ${ZELLIJ_CLAUDE_STALE_S:-900} / 60 ))
           running=""
           [ -f "$f.sess" ] && running=$(sh "$here/claude-agents.sh" "$(cat "$f.sess")" 2>/dev/null | head -n 1)
           # " away" = the terminal window showing this session is not the OS-focused window (os-focus.sh;
           # unknown counts as not away); the plugin then also dings/blinks the active tab.
           # ZELLIJ_CLAUDE_OS_FOCUS=0 skips the check.
           away=$(os_away)
           # a Stop while a question is still pending (interrupted): the owner is at the keyboard and the
           # attention ding already fired, so this finish is " quiet" (no second ding)
           if [ -e "$f.attn" ]; then away="$away quiet"; rm -f "$f.attn"; fi
           if [ -n "$running" ] || [ -n "$(find "$sub" -type f -mmin -"$stale_min" 2>/dev/null | head -n 1)" ]; then write done " sub$away"; else write done "$away"; fi ;;
  end)     rm -f "$f" "$f.ev" "$f.sess" "$f.attn"; rm -rf "$sub" "$f".once.* ;;
  substart|substop)
    [ -n "$2" ] || set -- "$1" "$(cat)"
    id=$(printf '%s' "$2" | sed -n 's/.*"agent_id"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1 | tr -c 'A-Za-z0-9_\n-' '_')
    [ -n "$id" ] || id="anon$$"
    if [ "$1" = substart ]; then
      mkdir -p "$sub" && : >"$sub/$id"
    else
      rm -f "$sub/$id"
      [ "$(sed -n 's/.* ag=//p' "$f.attn" 2>/dev/null)" = "$id" ] && rm -f "$f.attn"
    fi ;;
  *) exit 0 ;;
esac
# poke is best effort and must never delay or fail Claude
( run_timeout 3 zellij pipe --name claude_poke -- "" </dev/null >/dev/null 2>&1 & ) 2>/dev/null
exit 0
