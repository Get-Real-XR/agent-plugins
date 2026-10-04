#!/bin/bash

# PreToolUse hook: block Bash commands outside a jj repository, so project
# work stays under version control.
#
# With jj-workspace-guard enabled, stand down: it applies a finer rule
# outside repos, allowing reading, machine tools and making a repo. Without
# it, allow only commands that make or clone a jj repo.

source "$(dirname "$0")/guard-enabled.sh"
guard_enabled && exit 0

input=$(cat)
cwd=$(jq -r '.cwd // empty' <<<"$input")
(cd "${cwd:-.}" 2>/dev/null && jj root >/dev/null 2>&1) && exit 0

cmd=$(jq -r '.tool_input.command // empty' <<<"$input")
if [[ "$cmd" =~ (^|[\;\&\|\(][[:space:]]*)jj[[:space:]]+git[[:space:]]+(init|clone)([[:space:]]|$) ]]; then
  exit 0
fi

echo "Not in a jj repository. Project work happens in one: cd into the repo this belongs to, clone it with 'jj git clone --colocate <url> <dir>', or start one with 'mkdir -p <dir> && cd <dir> && jj git init'." >&2
exit 2
