#!/usr/bin/env bash
# Checks that the WorktreeCreate hook leaves the caller's change rewritable.
#
# With `immutable_heads()` including other workspaces' working copies (a
# common setting for parallel agents), a workspace created on top of the
# caller's working copy would freeze the caller's in-progress change. The
# hook must create it on the caller's parents instead.
#
# Usage: tests/create-keeps-caller-mutable.sh
set -euo pipefail

hook="$(cd "$(dirname "$0")/.." && pwd)/hooks/jj-worktree-create.sh"
t=$(mktemp -d)
trap 'rm -rf "$t"' EXIT
export JJ_CONFIG="$t/config.toml" JJ_WORKTREE_COMPAT_DIR="$t/workspaces"
cat >"$JJ_CONFIG" <<'EOF'
user.name = "probe"
user.email = "probe@example.com"
[revset-aliases]
"immutable_heads()" = "builtin_immutable_heads() | (working_copies() ~ @)"
EOF

jj git init "$t/repo" >/dev/null 2>&1
cd "$t/repo"
printf 'caller work\n' >caller.txt
jj describe -m 'caller: in progress' >/dev/null 2>&1

dest=$(jq -nc --arg cwd "$t/repo" '{name: "agent-1", cwd: $cwd}' | "$hook")
[ -d "$dest/.jj" ] || { echo "FAIL: no workspace at '$dest'"; exit 1; }

caller_parents=$(jj log --no-graph -r 'parents(default@)' -T 'commit_id ++ "\n"')
agent_parents=$(jj log --no-graph -r 'parents(agent-1@)' -T 'commit_id ++ "\n"')
[ "$caller_parents" = "$agent_parents" ] ||
  { echo "FAIL: the new workspace is not on the caller's parents"; exit 1; }

if ! jj describe -m 'caller: reworded' >/dev/null 2>&1; then
  echo "FAIL: the caller's change became immutable"
  exit 1
fi
echo "ok: workspace at $dest shares the caller's parents; the caller's change stays mutable"
