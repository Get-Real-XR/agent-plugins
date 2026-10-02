#!/usr/bin/env bash
set -euo pipefail

# WorktreeCreate hook for jj workspaces.
# Input:  JSON on stdin with { name, cwd, ... }
# Output: absolute path to the created workspace directory on stdout.
#
# Claude Code uses the path printed on stdout as the worktree-isolated agent's
# working directory. If this hook exits non-zero or prints no path, Claude Code
# falls back to the *caller's* cwd — so the agent silently lands in the shared
# checkout instead of an isolated worktree. Correctness therefore hinges on two
# rules: (1) print the path ONLY once the workspace genuinely exists, and
# (2) recover from transient failures instead of giving up.

input=$(cat)
name=$(echo "$input" | jq -r '.name')
cwd=$(echo "$input" | jq -r '.cwd')

# Resolve the jj workspace root from the caller's cwd. (If the caller is already
# inside a workspace, this resolves to that workspace; jj workspaces all share a
# single repo store, so `jj workspace add` still registers against the one repo.)
repo_root=$(cd "$cwd" && jj root)

# Place workspaces under <root>/.claude/worktrees/<name> — the location Claude
# Code expects for managed worktrees ("created under .claude/worktrees/ of this
# repository") — or under JJ_WORKTREE_COMPAT_DIR when set. An absolute
# directory gets a subdirectory per repo, so workspaces of different repos
# cannot collide. A relative directory is taken from the default workspace's
# root, so a caller that is itself in an added workspace does not nest new
# workspaces inside it; that workspace's `.jj/repo` is a file holding a path
# to the store, relative to its `.jj` directory.
store="$repo_root/.jj/repo"
if [ -f "$store" ]; then
  store=$(cd "$repo_root/.jj" && cd "$(cat repo)" && pwd)
fi
default_root=$(dirname "$(dirname "$store")")
dir=${JJ_WORKTREE_COMPAT_DIR:-.claude/worktrees}
case "$dir" in
  /*) worktree_base="$dir/$(basename "$default_root")" ;;
  *) worktree_base="$default_root/$dir" ;;
esac
dest="$worktree_base/$name"
mkdir -p "$worktree_base"

# Create the workspace, retrying transient failures. We deliberately do NOT take
# a global lock. Serializing every create makes cumulative working-copy checkout
# time grow with the number of concurrently spawned agents; in a large repo the
# later hooks can exceed Claude Code's hook timeout, get killed, and trigger the
# very "agent landed in the parent cwd" fallback this hook exists to prevent.
# jj resolves concurrent `jj workspace add` operations via op-log merge, so
# parallel creates are safe; a short retry loop covers rare op-log contention.
add_workspace() {
  (cd "$repo_root" && jj workspace add "$dest" --name "$name" -r @) 2>&1
}

err=""
for attempt in 1 2 3 4 5; do
  # If jj already tracks this name, the workspace is live (unique agent ids make
  # this rare outside of an idempotent re-invocation).
  if (cd "$repo_root" && jj workspace list 2>/dev/null) | grep -q "^${name}: "; then
    if [ -d "$dest/.jj" ]; then
      # Registered AND present on disk: hand back the path (idempotent retry).
      echo "$dest"
      exit 0
    fi
    echo "error: workspace '$name' is registered with jj but its directory is missing" >&2
    exit 1
  fi

  # A leftover directory with no jj registration is a stale remnant; clear it so
  # `jj workspace add` (which refuses a non-empty destination) can proceed.
  [ -e "$dest" ] && rm -rf "$dest"

  if err=$(add_workspace); then
    # Validate before committing to the path: print it only if the workspace
    # actually materialised on disk.
    if [ -d "$dest/.jj" ]; then
      echo "$dest"
      exit 0
    fi
  fi

  sleep 0.2
done

echo "error: failed to create jj workspace '$name' after 5 attempts" >&2
echo "$err" >&2
exit 1
