#!/bin/sh
# Print the ids of the background agents of a Claude Code session that are still running, one per line.
# Usage: claude-agents.sh SUBAGENTS_DIR     (= <transcript_path without .jsonl>/subagents)
# Source of truth is the per-agent transcript agent-<id>.jsonl. An agent is finished when its last message
# record (type user or assistant) is an assistant message with stop_reason end_turn, stop_sequence or
# refusal; anything else (tool_use, null = mid-stream, max_tokens, a user/tool_result line, a resumed agent's
# new user line), modified within the last 10 minutes, is running. Other records (api-request, attachment,
# prompt_snapshot, system, ...) are skipped: newer Claude Code versions append them after the final
# assistant message, so the LAST LINE is not reliable. Bounded: only the 40 newest files of the last 30 min,
# and only the last 16 lines of each (256 if those hold no message record).
# POSIX sh + awk; works with BSD find/tail/awk (macOS).
dir=$1
[ -d "$dir" ] || exit 0
# state of the last message record in the last $2 lines of $1: done | run | none (no message record seen)
agent_state() {
  tail -n "$2" "$1" 2>/dev/null | awk '
    # nested JSON in text is escaped (\"type\"), so these keys only match the record structure;
    # api-request-blob records carry "role":"assistant" but never "type":"assistant"
    /"type" *: *"assistant"/ && /"role" *: *"assistant"/ {
      s = "run"
      if ($0 ~ /"stop_reason" *: *"(end_turn|stop_sequence|refusal)"/) s = "done"
      next
    }
    /"type" *: *"user"/ { s = "run" }
    END { print (s == "" ? "none" : s) }'
}
find "$dir" -maxdepth 1 -type f -name 'agent-*.jsonl' -mmin -30 2>/dev/null | while read -r f; do
  printf '%s\n' "$f"
done | head -n 40 | while read -r f; do
  if [ -n "$(find "$f" -mmin -10 2>/dev/null)" ]; then
    st=$(agent_state "$f" 16)
    [ "$st" = none ] && st=$(agent_state "$f" 256)
    if [ "$st" != done ]; then
      b=${f##*/agent-}
      printf '%s\n' "${b%.jsonl}"
    fi
  fi
done
exit 0
