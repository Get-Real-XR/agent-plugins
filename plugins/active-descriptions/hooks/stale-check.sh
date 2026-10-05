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
# Check the workspaces the agent edited files in (see mark-writer.sh), wherever
# its shell happens to be: a shell that drifted into another agent's workspace
# must neither answer for that agent's change nor skip its own workspace.
# Sessions can share a workspace (a /fork starts in its parent's); one that
# edited nothing is not held. A subagent answers for its own edits; the main
# thread also answers for its subagents'.
#
# Block once per set of flagged changes. When the agent tries to stop again
# without describing them (Claude Code sets stop_hook_active), it has
# considered the request; blocking again would only loop.

revset=$1

source "$(dirname "$0")/guard-enabled.sh"
source "$(dirname "$0")/session-state.sh"
source "$(dirname "$0")/plugin-bin.sh"

IFS=$'\x1f' read -r session agent hook_active < <(
  jq -r '[.session_id // "", .agent_id // "main", (.stop_hook_active // false | tostring)] | join("\u001f")'
)

if [ -n "$session" ]; then
  state=$(session_state "$session")
  roots=$(awk -F'\t' -v agent="$agent" '
    (agent == "main" || $1 == agent) && !seen[$2]++ { print $2 }' "$state/writers" 2>/dev/null)
else
  roots=$(jj root 2>/dev/null)
fi
[ -n "$roots" ] || exit 0

bin=$(plugin_bin jj-stale-descriptions) || exit 0
msg=""
nl=$'\n'
while IFS= read -r root; do
  [ -d "$root/.jj" ] || continue
  # Only the default workspace holds `.jj/repo` as a directory; added
  # workspaces hold a file pointing to it.
  if [ -d "$root/.jj/repo" ] && guard_enabled; then
    continue
  fi
  found=$(cd "$root" && "$bin" "($revset) ~ ::(working_copies() ~ @)" 2>/dev/null) || continue
  [ -n "$found" ] && msg="${msg:+$msg$nl}In the workspace at $root:$nl$found"
done <<<"$roots"
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
echo "If a description still fits after a formatting-only or generated change, acknowledge it instead of rewording it, from that workspace: \"${CLAUDE_PLUGIN_ROOT}/hooks/ack.sh\" <change-id>" >&2
echo "If a flagged change is another session's unfinished work, leave it alone and say so; you will not be stopped twice for the same changes." >&2
exit 2
