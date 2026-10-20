#!/bin/sh
# Print the ids of the background agents of a Claude Code session that are still running, one per line.
# Usage: claude-agents.sh SUBAGENTS_DIR     (= <transcript_path without .jsonl>/subagents)
# Source of truth is the per-agent transcript agent-<id>.jsonl: an agent that ended its turn has a last line
# with "stop_reason":"end_turn"; anything else, modified within the last 10 minutes, is running (a resumed
# agent appends a user line, so it is running again). Bounded: only the 40 newest files of the last 30 min.
# POSIX sh; works with BSD find/tail (macOS).
dir=$1
[ -d "$dir" ] || exit 0
find "$dir" -maxdepth 1 -type f -name 'agent-*.jsonl' -mmin -30 2>/dev/null | while read -r f; do
  printf '%s\n' "$f"
done | head -n 40 | while read -r f; do
  if [ -n "$(find "$f" -mmin -10 2>/dev/null)" ]; then
    if ! tail -n 1 "$f" 2>/dev/null | grep -q '"stop_reason":"end_turn"'; then
      b=${f##*/agent-}
      printf '%s\n' "${b%.jsonl}"
    fi
  fi
done
exit 0
