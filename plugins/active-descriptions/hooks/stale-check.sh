#!/bin/bash

# Stop/SubagentStop hook: block stopping while a change in the revset given as
# $1 has a stale description.
#
# With jj-workspace-guard enabled, the default workspace is read-only for
# agents: one working there cannot describe anything, and the changes there
# are someone else's. Skip in that case instead of demanding the impossible.
#
# Check only this workspace's own changes: skip anything another workspace's
# working copy is built on. Those changes belong to whoever works there (the
# default workspace's stack, or another agent's stack this one branched from
# or interleaves with), and rewriting them moves that workspace's checkout.
#
# Hold only an agent that edited files in this workspace (see mark-writer.sh).
# Sessions can share a workspace: a /fork starts in its parent's. A subagent
# answers for its own edits; the main thread also answers for its subagents'.
#
# Block once per set of flagged changes. When the agent tries to stop again
# without describing them (Claude Code sets stop_hook_active), it has
# considered the request; blocking again would only loop.

revset=$1

source "$(dirname "$0")/guard-enabled.sh"
source "$(dirname "$0")/session-state.sh"

IFS=$'\x1f' read -r session agent hook_active < <(
  jq -r '[.session_id // "", .agent_id // "main", (.stop_hook_active // false | tostring)] | join("\u001f")'
)

root=$(jj root 2>/dev/null) || exit 0
# Only the default workspace holds `.jj/repo` as a directory; added
# workspaces hold a file pointing to it.
if [ -d "$root/.jj/repo" ] && guard_enabled; then
  exit 0
fi

if [ -n "$session" ]; then
  state=$(session_state "$session")
  awk -F'\t' -v agent="$agent" -v root="$root" '
    $2 == root && (agent == "main" || $1 == agent) { found = 1 }
    END { exit !found }' "$state/writers" 2>/dev/null || exit 0
fi

revset="($revset) ~ ::(working_copies() ~ @)"

msg=$(cargo run -r --manifest-path "${CLAUDE_PLUGIN_ROOT}/Cargo.toml" -- "$revset" 2>/dev/null) || exit 0
[ -n "$msg" ] || exit 0

if [ -n "$session" ]; then
  flagged=$(grep -oE 'Stale description: change [k-z]+' <<<"$msg" | sort -u)
  last_block="$state/$agent.last-block"
  if [ "$hook_active" = true ] && [ "$flagged" = "$(cat "$last_block" 2>/dev/null)" ]; then
    exit 0
  fi
  printf '%s\n' "$flagged" >"$last_block"
fi

echo "$msg" >&2
echo "If a description still fits after a formatting-only or generated change, acknowledge it instead of rewording it: \"${CLAUDE_PLUGIN_ROOT}/hooks/ack.sh\" <change-id>" >&2
echo "If a flagged change is another session's unfinished work in this workspace, leave it alone and say so; you will not be stopped twice for the same changes." >&2
exit 2
