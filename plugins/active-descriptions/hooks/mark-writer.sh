#!/bin/bash

# PostToolUse hook for file edits: record that this agent edited files in a
# jj workspace. The stop check holds an agent to the descriptions of a
# workspace's changes only if it edited files there; others sharing the
# workspace, such as a /fork that only answered a question, may stop.
#
# Records "<agent> <workspace root>" lines, tab-separated, in a file per
# session. The agent is the subagent's id, or "main" for the main thread.

source "$(dirname "$0")/session-state.sh"

IFS=$'\x1f' read -r session agent file < <(
  jq -r '[.session_id // "", .agent_id // "main",
          .tool_input.file_path // .tool_input.notebook_path // ""] | join("\u001f")'
)
[ -n "$session" ] && [ -n "$file" ] || exit 0
root=$(cd "$(dirname "$file")" 2>/dev/null && jj --ignore-working-copy root 2>/dev/null) || exit 0

state=$(session_state "$session")
line="$agent	$root"
grep -qxF "$line" "$state/writers" 2>/dev/null || printf '%s\n' "$line" >>"$state/writers"
