#!/usr/bin/env bash
set -euo pipefail

# WorktreeRemove hook for jj workspaces.
# Input: JSON on stdin with { worktree_path, cwd, ... }
# Output: ignored (exit 0 on success).

input=$(cat)
worktree_path=$(echo "$input" | jq -r '.worktree_path')
cwd=$(echo "$input" | jq -r '.cwd // empty')
name=$(basename "$worktree_path")

# No global lock: jj serialises its own repo mutations internally and resolves
# concurrent operations via op-log merge, so a remove is safe alongside other
# concurrent creates/removes.
#
# Forget the workspace from jj's registry, then remove its directory. Prefer
# forgetting by name from the repo root — that works even if the workspace
# directory is already gone or its .jj link is broken — and fall back to a
# self-forget from inside the workspace.
forgotten=false
if [ -n "$cwd" ]; then
  repo_root=$(cd "$cwd" 2>/dev/null && jj root 2>/dev/null || true)
  if [ -n "${repo_root:-}" ]; then
    (cd "$repo_root" && jj workspace forget "$name" 2>/dev/null) && forgotten=true || true
  fi
fi
if [ "$forgotten" = false ] && [ -d "$worktree_path/.jj" ]; then
  (cd "$worktree_path" && jj workspace forget "$name" 2>/dev/null) || true
fi

rm -rf "$worktree_path"
