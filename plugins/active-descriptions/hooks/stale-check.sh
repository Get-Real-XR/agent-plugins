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

revset=$1

source "$(dirname "$0")/guard-enabled.sh"

root=$(jj root 2>/dev/null) || exit 0
# Only the default workspace holds `.jj/repo` as a directory; added
# workspaces hold a file pointing to it.
if [ -d "$root/.jj/repo" ] && guard_enabled; then
  exit 0
fi
revset="($revset) ~ ::(working_copies() ~ @)"

msg=$(cargo run -r --manifest-path "${CLAUDE_PLUGIN_ROOT}/Cargo.toml" -- "$revset" 2>/dev/null) || exit 0
[ -n "$msg" ] || exit 0
echo "$msg" >&2
echo "If a description still fits after a formatting-only or generated change, acknowledge it instead of rewording it: \"${CLAUDE_PLUGIN_ROOT}/hooks/ack.sh\" <change-id>" >&2
exit 2
