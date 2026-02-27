#!/bin/bash

# PreToolUse hook: block tool calls unless inside a jj repository.
# Allows `jj git init --colocate` through so the user can bootstrap.

input=$(cat)

if echo "$input" | jq -e '.tool_input.command // empty' >/dev/null 2>&1; then
  cmd=$(echo "$input" | jq -r '.tool_input.command')
  if [[ "$cmd" =~ ^jj\ git\ init\ --colocate ]]; then
    exit 0
  fi
fi

jj root >/dev/null 2>&1 || {
  echo "Not in a jujutsu repository. Initialize with 'jj git init --colocate' first." >&2
  exit 2
}
