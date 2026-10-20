#!/bin/sh
# Claude Code hook -> zellij claude-tabs plugin.
# Usage: claude-hook.sh start|working|done|end|substart|substop|tool|ask|notify|clear   (hook JSON arrives on stdin)
# ask (PermissionRequest) / notify (Notification) / tool (PreToolUse AskUserQuestion, ExitPlanMode) raise the
# attention overlay <pane>.attn ("<ns> [away] ag=<agent id, main = the main agent>"); clear (PostToolUse, PostToolUseFailure),
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
# The claim is the directory font.claim (mkdir is atomic): one check at a time, also while font does not exist yet.
if [ -z "$(find "$root/font" -mmin -10 2>/dev/null)" ]; then
  if mkdir "$root/font.claim" 2>/dev/null \
     || { [ -n "$(find "$root/font.claim" -maxdepth 0 -mmin +1 2>/dev/null)" ] && rmdir "$root/font.claim" 2>/dev/null && mkdir "$root/font.claim" 2>/dev/null; }; then
    ( v=0
      case "$(uname -s 2>/dev/null)" in
        Darwin) ;;
        *) command -v fc-list >/dev/null 2>&1 && run_timeout 10 fc-list 'WC3 Icons' family 2>/dev/null | grep -qi 'WC3 Icons' && v=1 ;;
      esac
      printf '%s\n' "$v" >"$root/font.$$" && mv -f "$root/font.$$" "$root/font"
      rmdir "$root/font.claim" 2>/dev/null ) </dev/null >/dev/null 2>&1 &
  fi
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
# The helper runs in the background with every fd detached (a hung child can then never keep our pipe, or
# Claude Code's hook pipe, open) and its answer goes through a file; we wait at most ~1.5 s wall time and
# treat a missing answer as unknown.
os_away() {
  [ "${ZELLIJ_CLAUDE_OS_FOCUS:-1}" != 0 ] || return 0
  _of=$f.os.$$; rm -f "$_of" "$_of.done"
  ( run_timeout 1 sh "$here/os-focus.sh" "$ZELLIJ_SESSION_NAME" >"$_of" 2>/dev/null </dev/null; : >"$_of.done" ) </dev/null >/dev/null 2>&1 &
  _n=0; _max=15; _st=0.1
  sleep 0.1 2>/dev/null || { _st=1; _max=2; }
  while [ ! -e "$_of.done" ] && [ "$_n" -lt "$_max" ]; do sleep "$_st"; _n=$((_n + 1)); done
  _r=""; [ -e "$_of.done" ] && IFS= read -r _r <"$_of"
  rm -f "$_of" "$_of.done"
  [ "$_r" = unfocused ] && printf ' away'
  return 0
}
# attention: raise the overlay unless one is already pending (PermissionRequest, Notification and PreToolUse
# of one request must ding once). <pane>.attn-seen is a tombstone of the last raise ("<ns> ag=<agent>"): the
# plugin deletes .attn when the tab is in view, and a duplicate signal for that same request (the Notification
# arrives after the PermissionRequest) must not raise it again, so within 10 s of the raise and until a
# clearing event of the asking agent it is ignored.
agent_of() { _a=$(json_str agent_id "$1"); printf '%s' "${_a:-main}"; }
attn_raise() {
  [ -e "$f.attn" ] && return 0
  if [ -e "$f.attn-seen" ]; then
    _s=$(cut -d' ' -f1 "$f.attn-seen" 2>/dev/null)
    case $_s in ''|*[!0-9]*) _s=0 ;; esac
    [ $(( $(now_ns) - _s )) -lt 10000000000 ] && return 0
    rm -f "$f.attn-seen"
  fi
  [ -e "$f" ] || write idle
  ag=$(agent_of "$2")
  _ns=$(now_ns)
  # link(2) never replaces: of two concurrent raises exactly one overlay (one ns, one ding) survives
  printf '%s%s ag=%s\n' "$_ns" "$(os_away)" "$ag" >"$f.attn.$$" && { ln "$f.attn.$$" "$f.attn" 2>/dev/null; rm -f "$f.attn.$$"; }
  printf '%s ag=%s\n' "$_ns" "$ag" >"$f.attn-seen"
  : >"$f.ev"
}
# attn_drop PAYLOAD: remove the overlay (and the tombstone) when the event comes from the agent that asked,
# or the asker is unknown (legacy overlay "ag=-"); a subagent's tool call leaves the main agent's request alone.
# Returns 0 when the overlay itself was removed.
attn_drop() {
  _ag=$(agent_of "$1"); _r=1
  for _x in attn attn-seen; do
    [ -e "$f.$_x" ] || continue
    _w=$(sed -n 's/.* ag=\(.*\)$/\1/p' "$f.$_x" 2>/dev/null | head -n 1)
    if [ -z "$_w" ] || [ "$_w" = - ] || [ "$_w" = "$_ag" ]; then
      rm -f "$f.$_x"; [ "$_x" = attn ] && _r=0
    fi
  done
  return $_r
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
# any event except the end of the session counts as activity (the plugin's `working_hold_s` safety net);
# substop only when it stops a known agent (below): Claude Code sends a SubagentStop without SubagentStart and
# with an empty agent_type for an internal helper ~2 s after the main Stop, which must not look like work
case "$1" in end|notify|ask|clear|substop) ;; *) : >"$f.ev" ;; esac
case "$1" in
  tool)    # optional PreToolUse/PostToolUse hook: a turn that started without UserPromptSubmit (the main
           # agent woken by a finished subagent) is working
           # AskUserQuestion / ExitPlanMode wait for the user: attention. Any other tool call of the asker
           # means the question was answered (a denied permission fires no hook of its own).
           case "$(json_str tool_name "$2")" in
             AskUserQuestion|ExitPlanMode) attn_raise "$@"; ( run_timeout 3 zellij pipe --name claude_poke -- "" </dev/null >/dev/null 2>&1 & ) 2>/dev/null; exit 0 ;;
           esac
           attn_drop "$2"
           cur=$(cut -d' ' -f1 "$f" 2>/dev/null)
           [ "$cur" = working ] || write working
           exit 0 ;;
  ask|notify) attn_raise "$@" ;;
  clear)   # PostToolUse / PostToolUseFailure: cheap no-op unless an attention request is pending or the
           # tool was TaskStop (a stopped background agent sends no SubagentStop: drop its marker here)
           _p=""
           if [ "$(json_str tool_name "$2")" = TaskStop ]; then
             _t=$(json_str task_id "$2" | tr -c 'A-Za-z0-9_\n-' '_')
             [ -n "$_t" ] && [ -e "$sub/$_t" ] && rm -f "$sub/$_t" && _p=1
           fi
           if [ -e "$f.attn" ] || [ -e "$f.attn-seen" ]; then attn_drop "$2" && : >"$f.ev" && _p=1; fi
           [ -n "$_p" ] || exit 0 ;;
  start)   rm -rf "$sub"; rm -f "$f.attn" "$f.attn-seen"; write idle ;;
  working) rm -f "$f.attn" "$f.attn-seen"; write working ;;
  done)    # background subagents may outlive the main turn: keep their markers, and tell the plugin that
           # this Stop is not a real finish while a (non-stale) subagent marker exists
           stale_s=${ZELLIJ_CLAUDE_STALE_S:-900}
           case $stale_s in ''|*[!0-9]*) stale_s=900 ;; esac
           stale_min=$(( stale_s / 60 ))
           running=""
           # drop the markers of agents whose transcript says finished (killed by the user / TaskStop:
           # no SubagentStop ever arrives, the marker would count as running for ZELLIJ_CLAUDE_STALE_S)
           if [ -f "$f.sess" ] && [ -d "$sub" ]; then
             sh "$here/claude-agents.sh" --finished "$(cat "$f.sess")" "$sub" 2>/dev/null | while read -r _i; do rm -f "$sub/$_i"; done
           fi
           [ -f "$f.sess" ] && running=$(sh "$here/claude-agents.sh" "$(cat "$f.sess")" 2>/dev/null | head -n 1)
           # " away" = the terminal window showing this session is not the OS-focused window (os-focus.sh;
           # unknown counts as not away); the plugin then also dings/blinks the active tab.
           # ZELLIJ_CLAUDE_OS_FOCUS=0 skips the check.
           away=$(os_away)
           # a Stop while a question of the main agent is still pending (interrupted, raised < 60 s ago): the user
           # is at the keyboard and the attention ding already fired, so this finish is " quiet" (no second
           # ding). A stale or foreign overlay is just dropped and the finish dings normally.
           if [ -e "$f.attn" ]; then
             _l=$(cat "$f.attn" 2>/dev/null)
             _ans=${_l%% *}; _wa=${_l##* ag=}
             case $_ans in ''|*[!0-9]*) _ans=0 ;; esac
             if [ $(( $(now_ns) - _ans )) -lt 60000000000 ] && { [ "$_wa" = main ] || [ "$_wa" = - ]; }; then away="$away quiet"; fi
             rm -f "$f.attn"
           fi
           rm -f "$f.attn-seen"
           if [ -n "$running" ] || [ -n "$(find "$sub" -type f -mmin -"$stale_min" 2>/dev/null | head -n 1)" ]; then write done " sub$away"; else write done "$away"; fi ;;
  end)     rm -f "$f" "$f.ev" "$f.sess" "$f.attn" "$f.attn-seen"; rm -rf "$sub" "$f".once.* ;;
  substart|substop)
    [ -n "$2" ] || set -- "$1" "$(cat)"
    id=$(printf '%s' "$2" | sed -n 's/.*"agent_id"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1 | tr -c 'A-Za-z0-9_\n-' '_')
    [ -n "$id" ] || id="anon$$"
    if [ "$1" = substart ]; then
      mkdir -p "$sub" && : >"$sub/$id"
    else
      if [ -e "$sub/$id" ] || [ -n "$(json_str agent_type "$2")" ]; then : >"$f.ev"; fi
      rm -f "$sub/$id"
      [ "$(sed -n 's/.* ag=//p' "$f.attn" 2>/dev/null)" = "$id" ] && rm -f "$f.attn"
      [ "$(sed -n 's/.* ag=//p' "$f.attn-seen" 2>/dev/null)" = "$id" ] && rm -f "$f.attn-seen"
    fi ;;
  *) exit 0 ;;
esac
# poke is best effort and must never delay or fail Claude
( run_timeout 3 zellij pipe --name claude_poke -- "" </dev/null >/dev/null 2>&1 & ) 2>/dev/null
exit 0
