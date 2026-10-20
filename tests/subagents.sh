#!/bin/sh
# Subagent bookkeeping of claude-hook.sh / claude-agents.sh with simulated hook payloads.
# Usage: sh tests/subagents.sh      (exit 0 = all passed)
# Runs in a private TMPDIR (own zellij-<uid>/zellij-log so the state root resolves there, never to a live
# session) with a stub `zellij` on PATH (no poke reaches a real session).
here=$(cd "$(dirname "$0")/.." && pwd -P)
T=$(mktemp -d "${TMPDIR:-/tmp}/wc3-test.XXXXXX") || exit 1
trap 'rm -rf "$T"' EXIT
mkdir -p "$T/zellij-$(id -u)/zellij-log" "$T/bin" "$T/proj/s1/subagents"
printf '#!/bin/sh\nexit 0\n' >"$T/bin/zellij"; chmod +x "$T/bin/zellij"
export TMPDIR="$T" PATH="$T/bin:$PATH" ZELLIJ=0 ZELLIJ_SESSION_NAME=test ZELLIJ_PANE_ID=1 ZELLIJ_CLAUDE_OS_FOCUS=0
st="$T/zellij-$(id -u)/zellij-claude/test"
f="$st/1"
sa="$T/proj/s1/subagents"
tp="\"transcript_path\":\"$T/proj/s1.jsonl\""
fail=0
ok() { if eval "$2"; then echo "ok   $1"; else echo "FAIL $1"; fail=1; fi; }
hook() { printf '%s' "$2" | sh "$here/claude-hook.sh" "$1"; }
# transcripts: finished, interrupted (TaskStop / killed by the user), running (last message is a tool call)
done_rec='{"type":"assistant","message":{"role":"assistant","stop_reason":"end_turn","content":[{"type":"text","text":"ok"}]}}'
intr_rec='{"type":"user","message":{"role":"user","content":[{"type":"text","text":"[Request interrupted by user]"}]}}'
run_rec='{"type":"assistant","message":{"role":"assistant","stop_reason":"tool_use","content":[{"type":"tool_use","name":"Bash"}]}}'
tail_rec='{"type":"api-request","requestId":"x"}'
transcript() { printf '%s\n%s\n' "$2" "$tail_rec" >"$sa/agent-$1.jsonl"; }

hook start "{$tp}"
# 1. TaskStop (PostToolUse) drops the stopped agent's marker
hook substart "{$tp,\"agent_id\":\"a1\",\"agent_type\":\"general-purpose\"}"
ok "substart writes a marker" '[ -e "$f.sub/a1" ]'
transcript a1 "$intr_rec"
hook clear "{$tp,\"tool_name\":\"TaskStop\",\"tool_input\":{\"task_id\":\"a1\"},\"tool_response\":{}}"
ok "TaskStop removes the marker" '[ ! -e "$f.sub/a1" ]'
# 2. an interrupted agent without TaskStop: the Stop drops its marker and is a real finish
hook substart "{$tp,\"agent_id\":\"a2\",\"agent_type\":\"general-purpose\"}"
sleep 1; transcript a2 "$intr_rec"
ok "claude-agents.sh: interrupted agent is not running" '[ -z "$(sh "$here/claude-agents.sh" "$sa")" ]'
hook done "{$tp,\"hook_event_name\":\"Stop\"}"
ok "Stop drops a finished agent's marker" '[ ! -e "$f.sub/a2" ]'
ok "Stop without running agents is a finish" '[ "$(cut -d" " -f1 "$f") $(cut -d" " -f3 "$f")" = "done " ]'
# 3. a running agent keeps its marker and defers the finish
transcript a3 "$run_rec"
hook substart "{$tp,\"agent_id\":\"a3\",\"agent_type\":\"general-purpose\"}"
ok "claude-agents.sh: tool_use agent is running" '[ "$(sh "$here/claude-agents.sh" "$sa")" = a3 ]'
hook done "{$tp,\"hook_event_name\":\"Stop\",\"n\":2}"
ok "Stop keeps a running agent's marker" '[ -e "$f.sub/a3" ]'
ok "Stop with a running agent is deferred (sub)" '[ "$(cut -d" " -f3 "$f")" = sub ]'
# 4. a resumed agent (marker newer than its finished transcript) keeps its marker
transcript a4 "$done_rec"; touch -t 202001010000 "$sa/agent-a4.jsonl"
hook substart "{$tp,\"agent_id\":\"a4\",\"agent_type\":\"general-purpose\"}"
hook done "{$tp,\"hook_event_name\":\"Stop\",\"n\":3}"
ok "Stop keeps the marker of a resumed agent" '[ -e "$f.sub/a4" ]'
exit $fail
