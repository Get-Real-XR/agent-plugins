#!/usr/bin/env bash
set -euo pipefail

# WorktreeCreate hook for jj workspaces.
# Input: JSON on stdin with { name, cwd, ... }
# Output: absolute path to created workspace directory on stdout

input=$(cat)
name=$(echo "$input" | jq -r '.name')
cwd=$(echo "$input" | jq -r '.cwd')

# Find the jj repo root from the session's cwd
repo_root=$(cd "$cwd" && jj root)

# Place workspaces under ~/.claude/worktrees/{name}
dest="$HOME/.claude/worktrees/$name"

# Clean up stale workspace at this path if it exists
if [ -d "$dest" ]; then
  (cd "$repo_root" && jj workspace forget "$name" 2>/dev/null) || true
  rm -rf "$dest"
fi

mkdir -p "$(dirname "$dest")"

# Create the workspace. New working-copy commit shares the same parents
# as the main workspace's @, so the agent starts with identical code.
(cd "$repo_root" && jj workspace add "$dest" --name "$name") >&2

# Stdout = absolute path (the hook contract)
echo "$dest"
