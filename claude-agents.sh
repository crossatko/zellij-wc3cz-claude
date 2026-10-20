#!/bin/sh
# Print the ids of the background agents of a Claude Code session that are still running, one per line.
# Usage: claude-agents.sh SUBAGENTS_DIR     (= <transcript_path without .jsonl>/subagents)
#        claude-agents.sh --finished SUBAGENTS_DIR MARKER_DIR
#   prints the names of the hook markers in MARKER_DIR (<pane>.sub, one file per agent id) whose transcript
#   says finished and is not older than the marker (the hook's Stop drops them: an agent killed with
#   TaskStop or by the user never sends SubagentStop).
# Source of truth is the per-agent transcript agent-<id>.jsonl. An agent is finished when its last message
# record (type user or assistant) is an assistant message with stop_reason end_turn, stop_sequence or
# refusal, a user record "[Request interrupted by user...]" (TaskStop / killed by the user: no
# SubagentStop follows), or the tool_result of a SubagentHandback tool call (Claude Code 2.1.301+: an agent
# can end by handing its report back with that tool, no end_turn follows); anything else (tool_use, null = mid-stream, max_tokens, a user/tool_result line, a resumed agent's
# new user line), modified within the last 10 minutes, is running. Other records (api-request, attachment,
# prompt_snapshot, system, ...) are skipped: newer Claude Code versions append them after the final
# assistant message, so the LAST LINE is not reliable. Bounded: only the 40 newest files of the last 30 min,
# and only the last 16 lines of each (256 if those hold no message record).
# POSIX sh + awk; works with BSD find/tail/awk (macOS).
# state of the last message record in the last $2 lines of $1: done | run | none (no message record seen)
agent_state() {
  tail -n "$2" "$1" 2>/dev/null | awk '
    # nested JSON in text is escaped (\"type\"), so these keys only match the record structure;
    # api-request-blob records carry "role":"assistant" but never "type":"assistant"
    /"type" *: *"assistant"/ && /"role" *: *"assistant"/ {
      s = "run"
      if ($0 ~ /"stop_reason" *: *"(end_turn|stop_sequence|refusal)"/) s = "done"
      else if ($0 ~ /"type" *: *"tool_use"/ && $0 ~ /"name" *: *"SubagentHandback"/) s = "hb"
      next
    }
    # the tool_result of a SubagentHandback call ends the agent (no end_turn message follows)
    /"type" *: *"user"/ { s = ($0 ~ /"text" *: *"\[Request interrupted by user/ || (s == "hb" && $0 ~ /"type" *: *"tool_result"/)) ? "done" : "run" }
    END { print (s == "" ? "none" : (s == "hb" ? "run" : s)) }'
}
# finished = done, also when the last 16 lines hold no message record
finished() {
  _st=$(agent_state "$1" 16)
  [ "$_st" = none ] && _st=$(agent_state "$1" 256)
  [ "$_st" = done ]
}
if [ "$1" = --finished ]; then
  [ -d "$2" ] && [ -d "$3" ] || exit 0
  for m in "$3"/*; do
    [ -f "$m" ] || continue
    t="$2/agent-${m##*/}.jsonl"
    [ -f "$t" ] && ! [ "$m" -nt "$t" ] && finished "$t" && printf '%s\n' "${m##*/}"
  done
  exit 0
fi
dir=$1
[ -d "$dir" ] || exit 0
find "$dir" -maxdepth 1 -type f -name 'agent-*.jsonl' -mmin -30 2>/dev/null | while read -r f; do
  printf '%s\n' "$f"
done | head -n 40 | while read -r f; do
  if [ -n "$(find "$f" -mmin -10 2>/dev/null)" ]; then
    if ! finished "$f"; then
      b=${f##*/agent-}
      printf '%s\n' "${b%.jsonl}"
    fi
  fi
done
exit 0
