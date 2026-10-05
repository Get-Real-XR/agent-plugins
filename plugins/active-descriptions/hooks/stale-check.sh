#!/bin/bash

# Stop/SubagentStop hook: flag the changes an agent edited whose descriptions
# are out of date, without blocking. The agent gets a note at the start of its
# next turn (prompt-note.sh), the user sees one line now, and push-check.sh
# refuses to push such changes.
#
# Blocking at every turn's end made agents rewrite descriptions of work still
# in progress, and stopped agents over other sessions' changes: a /fork in
# its parent's workspace, or a shell that had drifted into another agent's.
#
# $1 is the revset to check in each workspace the agent edited files in (see
# mark-writer.sh), wherever its shell is now; an agent that edited nothing is
# not flagged. A set of flagged changes is noted once, until it changes or
# every change in it is described again.

revset=$1

source "$(dirname "$0")/session-state.sh"
source "$(dirname "$0")/stale-report.sh"

IFS=$'\x1f' read -r session agent < <(
  jq -r '[.session_id // "", .agent_id // "main"] | join("\u001f")'
)
[ -n "$session" ] || exit 0
state=$(session_state "$session")

report=$(edited_roots "$state" "$agent" | stale_report "$revset")
noted="$state/$agent.noted"
if [ -z "$report" ]; then
  rm -f "$noted"
  exit 0
fi
flagged=$(flagged_changes "$report")
[ "$flagged" = "$(cat "$noted" 2>/dev/null)" ] && exit 0
printf '%s\n' "$flagged" >"$noted"

# The note is for the main thread's next turn; a subagent ends here, so its
# note goes to the thread that outlives it.
printf '%s\n' "$report" >>"$state/pending-note"

if [ "$agent" = main ]; then
  jq -nc --arg changes "$(paste -sd ' ' - <<<"$flagged")" '{systemMessage: (
    "active-descriptions: changes edited this session have out-of-date descriptions (" + $changes +
    "). The agent is reminded on its next turn, and jj git push refuses them until they are described.")}'
fi
exit 0
