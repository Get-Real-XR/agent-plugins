#!/usr/bin/env bash
# Checks whom the stop hook holds to which workspace's descriptions, and how
# often, with simulated hook input. No guard settings apply (HOME is fresh).
#
# Usage: tests/stop-scope.sh
set -euo pipefail

plugin="$(cd "$(dirname "$0")/.." && pwd)"
t=$(mktemp -d)
t=$(cd "$t" && pwd -P)
trap 'rm -rf "$t"' EXIT
# Keep the real toolchain and build cache; everything else is fresh.
export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}" CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
export XDG_CACHE_HOME="${XDG_CACHE_HOME:-$HOME/.cache}"
export HOME="$t/home" XDG_STATE_HOME="$t/state" CLAUDE_PLUGIN_ROOT="$plugin" JJ_CONFIG="$t/config.toml"
mkdir -p "$HOME"
printf 'user.name = "probe"\nuser.email = "probe@example.com"\n' >"$JJ_CONFIG"
"$plugin/hooks/build-bin.sh" jj-stale-descriptions
failed=0

repo="$t/repo"
jj git init "$repo" >/dev/null 2>&1
mkdir -p "$repo/workspaces"
for name in a b c; do
  jj -R "$repo" workspace add --name "$name" "$repo/workspaces/$name" >/dev/null 2>&1
done
a="$repo/workspaces/a" b="$repo/workspaces/b" c="$repo/workspaces/c"

edit() { # session agent file
  printf 'edit\n' >>"$3"
  jq -nc --arg s "$1" --arg a "$2" --arg f "$3" \
    '{session_id: $s, tool_name: "Write", tool_input: {file_path: $f}} + (if $a == "main" then {} else {agent_id: $a} end)' |
    "$plugin/hooks/mark-writer.sh"
}
expect() { # label want-exit dir session agent active revset [must-mention] [must-not-mention]
  local out code
  out=$(cd "$3" && jq -nc --arg s "$4" --arg a "$5" --argjson act "$6" \
    '{session_id: $s, stop_hook_active: $act} + (if $a == "main" then {} else {agent_id: $a} end)' |
    "$plugin/hooks/stale-check.sh" "$7" 2>&1) && code=0 || code=$?
  local ok=true
  [ "$code" = "$2" ] || ok=false
  [ -z "${8:-}" ] || grep -qF "$8" <<<"$out" || ok=false
  [ -z "${9:-}" ] || ! grep -qF "$9" <<<"$out" || ok=false
  if $ok; then echo "ok   $1"; else echo "FAIL $1 (exit $code): $out"; failed=1; fi
}
main='trunk()..@ ~ empty()'
sub='default@..@ ~ empty()'
change_of() { jj -R "$1" log --no-graph -r @ -T 'change_id.short(12)'; }

expect 'a session that never edited stops' 0 "$a" fork main false "$main"
edit writer main "$a/notes.txt"
expect 'a session that edited its workspace is held' 2 "$a" writer main false "$main"
expect '...and let go on its second attempt' 0 "$a" writer main true "$main"
edit writer main "$a/notes.txt"
expect '...also after its diff grows' 0 "$a" writer main true "$main"
(cd "$a" && jj describe -m notes >/dev/null 2>&1 && jj new >/dev/null 2>&1)
edit writer main "$a/next.txt"
expect '...but held again for a new change' 2 "$a" writer main true "$main"
edit sub-session sub1 "$c/sub.txt"
expect 'a subagent that edited is held' 2 "$c" sub-session sub1 false "$sub"
expect 'another subagent of that session is not' 0 "$c" sub-session sub2 false "$sub"
expect 'the main thread answers for its subagent' 2 "$c" sub-session main false "$main"

# Drift: the shell ends up in another agent's workspace.
printf 'theirs\n' >"$b/theirs.txt"
(cd "$b" && jj status >/dev/null 2>&1)
theirs=$(change_of "$b")
edit drifter main "$c/clean.txt"
(cd "$c" && jj describe -m 'clean work' >/dev/null 2>&1 && jj new >/dev/null 2>&1)
expect "a drifted shell does not answer for the other agent's change" 0 "$b" drifter main false "$main"
edit drifter2 main "$c/stale.txt"
mine=$(change_of "$c")
expect 'a drifted shell is still held for its own workspace' 2 "$b" drifter2 main false "$main" "$mine" "$theirs"

exit "$failed"
