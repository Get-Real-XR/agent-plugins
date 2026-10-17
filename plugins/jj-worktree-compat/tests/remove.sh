#!/usr/bin/env bash
# Checks that the WorktreeRemove hook removes a colocated workspace
# completely: jj no longer lists it, its directory and Git worktree are gone,
# and an edit made after the agent's last jj command survives in a commit.
# Covers a caller in the repo or in the workspace, a directory that is
# already gone, and jj before 0.46, which has no `jj workspace remove`.
#
# Needs jj 0.46 or later, which colocates added workspaces.
#
# Usage: tests/remove.sh
set -euo pipefail

hooks="$(cd "$(dirname "$0")/.." && pwd)/hooks"
t=$(mktemp -d)
t=$(cd "$t" && pwd -P)
trap 'rm -rf "$t"' EXIT
export JJ_CONFIG="$t/config.toml" JJ_WORKTREE_COMPAT_DIR="$t/workspaces"
printf 'user.name = "probe"\nuser.email = "probe@example.com"\n' >"$JJ_CONFIG"
fail() { echo "FAIL: $*"; exit 1; }

# Stands in for jj before 0.46 by refusing `jj workspace remove`.
mkdir -p "$t/old-jj"
cat >"$t/old-jj/jj" <<EOF
#!/usr/bin/env bash
if [ "\${1:-}" = workspace ] && [ "\${2:-}" = remove ]; then
  echo "error: unrecognized subcommand 'remove'" >&2
  exit 2
fi
exec "$(command -v jj)" "\$@"
EOF
chmod +x "$t/old-jj/jj"

repo="$t/repo"
jj git init "$repo" >/dev/null 2>&1
(cd "$repo" && echo base >base.txt && jj commit -m base >/dev/null 2>&1)

check() { # name, caller (repo|workspace), directory (kept|gone), jj (new|old)
  local name=$1 caller=$2 directory=$3 version=$4 dest cwd path=$PATH
  dest=$(jq -nc --arg name "$name" --arg cwd "$repo" '{name: $name, cwd: $cwd}' |
    "$hooks/jj-worktree-create.sh")
  [ -f "$dest/.git" ] || fail "$name: the new workspace has no Git worktree"
  printf '%s\n' "$name" >"$dest/edit-$name.txt"
  [ "$directory" = gone ] && rm -rf "$dest"
  if [ "$caller" = workspace ]; then cwd=$dest; else cwd=$repo; fi
  [ "$version" = old ] && path="$t/old-jj:$PATH"

  jq -nc --arg path "$dest" --arg cwd "$cwd" '{worktree_path: $path, cwd: $cwd}' |
    (cd "$t" && PATH=$path "$hooks/jj-worktree-remove.sh")

  [ ! -e "$dest" ] || fail "$name: the directory is still there"
  if (cd "$repo" && jj workspace list -T 'name ++ "\n"') | grep -qxF "$name"; then
    fail "$name: jj still lists the workspace"
  fi
  if git -C "$repo" worktree list --porcelain | grep -qxF "worktree $dest"; then
    fail "$name: Git still lists the worktree"
  fi
  if [ "$directory" = kept ] &&
    [ -z "$(cd "$repo" && jj log --no-graph -r "files(\"edit-$name.txt\")" -T 'change_id')" ]; then
    fail "$name: the edit made after the last jj command was lost"
  fi
  echo "ok: $name"
}

check from-repo repo kept new
check from-workspace workspace kept new
check directory-gone repo gone new
check old-jj repo kept old
