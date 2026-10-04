#!/usr/bin/env bash
# Checks workspace-lifecycle.sh: which workspaces a session's end removes,
# and what a sweep lists and removes.
#
# Usage: tests/lifecycle.sh
set -euo pipefail

script="$(cd "$(dirname "$0")/.." && pwd)/hooks/workspace-lifecycle.sh"
t=$(mktemp -d)
t=$(cd "$t" && pwd -P)
trap 'rm -rf "$t"' EXIT
export HOME="$t/home" XDG_STATE_HOME="$t/state" JJ_CONFIG="$t/config.toml"
mkdir -p "$HOME/.claude/projects/p"
printf 'user.name = "probe"\nuser.email = "probe@example.com"\n' >"$JJ_CONFIG"
fail() { echo "FAIL: $*"; exit 1; }

repo="$t/repo"
jj git init "$repo" >/dev/null 2>&1
ws="$HOME/workspaces/repo"
mkdir -p "$ws"

# An agent adds workspaces; the PostToolUse hook sees each command.
add() { # name, command as the agent wrote it
  (cd "$repo" && jj workspace add --name "$1" "$ws/$1" >/dev/null 2>&1)
  jq -nc --arg c "$2" --arg cwd "$repo" \
    '{session_id: "s1", cwd: $cwd, tool_name: "Bash", tool_input: {command: $c}}' |
    "$script" track
}
add idle 'jj workspace add --name idle $HOME/workspaces/repo/idle'
add busy "cd $repo && jj workspace add --name busy ~/workspaces/repo/busy"
add shared "jj workspace add \"$ws/shared\" -r @-"
add gone "jj workspace add --name gone $ws/gone"
add occupied "jj workspace add --name occupied $ws/occupied"
add visited "jj workspace add --name visited $ws/visited"
(cd "$repo" && jj workspace add --name other "$ws/other" >/dev/null 2>&1)

jq -nc '{session_id: "s1", tool_name: "Bash", tool_input: {command: "ls -la"}}' | "$script" track
[ "$(wc -l <"$XDG_STATE_HOME/jj-worktree-compat/sessions/s1")" -eq 6 ] ||
  fail "expected 6 tracked workspaces, got: $(cat "$XDG_STATE_HOME/jj-worktree-compat/sessions/s1")"

printf 'work\n' >"$ws/busy/notes.txt"
printf '{"cwd":"%s","type":"user"}\n' "$ws/shared" >"$HOME/.claude/projects/p/s2.jsonl"
printf '{"cwd":"%s"}\n{"cwd":"%s"}\n' "$ws/visited" "$repo" >"$HOME/.claude/projects/p/s3.jsonl"
(cd "$repo" && jj workspace forget gone >/dev/null 2>&1)
(cd "$ws/occupied" && exec sleep 60) &
occupant=$!

"$script" cleanup-session s1
[ ! -e "$ws/idle" ] || fail "the idle workspace was kept"
[ ! -e "$ws/gone" ] || fail "the forgotten folder was kept"
[ ! -e "$ws/visited" ] || fail "a workspace another session only visited earlier was kept"
[ -d "$ws/busy" ] || fail "the workspace with work was removed"
[ -d "$ws/shared" ] || fail "the workspace another session uses was removed"
[ -d "$ws/other" ] || fail "a workspace this session did not add was removed"
[ -d "$ws/occupied" ] || fail "a workspace a process works in was removed"
kill "$occupant"; wait "$occupant" 2>/dev/null || true
(cd "$repo" && jj workspace list -T 'name ++ "\n"') | grep -qx idle && fail "idle is still listed"
grep -qx "$ws/busy" "$XDG_STATE_HOME/jj-worktree-compat/sessions/s1" || fail "busy is no longer tracked"
echo "ok: session end removed the idle, visited and forgotten workspaces, kept busy, shared, occupied and other"

# A sweep: everything was just used, so nothing is old enough.
out=$("$script" sweep "$repo")
grep -q "^recent .*/other$" <<<"$out" || fail "sweep did not report other as recent: $out"

# Age the workspaces; the shared one is still in use.
for name in busy shared other; do
  find "$ws/$name/.jj/working_copy" -exec touch -t 202601010000 {} +
done
(cd "$repo" && jj workspace add --name left "$t/repo/workspaces/left" >/dev/null 2>&1 || {
  mkdir -p "$t/repo/workspaces" && jj workspace add --name left "$t/repo/workspaces/left" >/dev/null 2>&1; })
(cd "$repo" && jj workspace forget left >/dev/null 2>&1)
out=$("$script" sweep "$repo")
grep -q "^idle .*/other$" <<<"$out" || fail "sweep did not list other as idle: $out"
grep -q "^has work .*/busy$" <<<"$out" || fail "sweep did not keep busy: $out"
grep -q "^in use .*/shared$" <<<"$out" || fail "sweep did not keep shared: $out"
grep -q "^forgotten .*/left " <<<"$out" || fail "sweep did not list the forgotten folder: $out"
"$script" sweep "$repo" --apply >/dev/null
[ ! -e "$ws/other" ] || fail "sweep --apply kept the idle workspace"
[ -d "$t/repo/workspaces/left" ] || fail "sweep --apply removed a forgotten folder without --forgotten"
"$script" sweep "$repo" --apply --forgotten >/dev/null
[ ! -e "$t/repo/workspaces/left" ] || fail "sweep --apply --forgotten kept the forgotten folder"
[ -d "$ws/busy" ] && [ -d "$ws/shared" ] || fail "sweep removed a workspace it should keep"
echo "ok: sweep lists idle, busy, in-use and forgotten workspaces, and removes only what it should"
