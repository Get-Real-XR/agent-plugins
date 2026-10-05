#!/bin/bash

# UserPromptSubmit hook: hand the agent, once, the note stale-check.sh left at
# the end of its last turn about changes with out-of-date descriptions.

source "$(dirname "$0")/session-state.sh"

session=$(jq -r '.session_id // ""')
[ -n "$session" ] || exit 0
note="$(session_state "$session")/pending-note"
[ -s "$note" ] || exit 0
report=$(cat "$note")
rm -f "$note"

jq -nc --arg report "$report" '{hookSpecificOutput: {
  hookEventName: "UserPromptSubmit",
  additionalContext: (
    "active-descriptions: when your last turn ended, these changes you edited had out-of-date descriptions:\n" +
    $report +
    "\nUpdate them with the describe skill when this work reaches a stopping point, not mid-task. " +
    "Skip any that are already up to date, or that are another session'"'"'s unfinished work. " +
    "jj git push refuses to push a change whose description is out of date.")}}'
