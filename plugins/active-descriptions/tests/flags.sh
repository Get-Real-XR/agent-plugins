#!/usr/bin/env bash
# Checks the out-of-date description hooks with simulated hook input: what
# the end of a turn flags, and for whom; the note on the next turn; and which
# pushes are refused. No guard settings apply (HOME is fresh).
#
# Usage: tests/flags.sh
set -euo pipefail

plugin="$(cd "$(dirname "$0")/.." && pwd)"
hooks="$plugin/hooks"
t=$(mktemp -d)
t=$(cd "$t" && pwd -P)
trap 'rm -rf "$t"' EXIT
# Keep the real toolchain and build cache; everything else is fresh.
export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}" CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
export XDG_CACHE_HOME="${XDG_CACHE_HOME:-$HOME/.cache}"
export HOME="$t/home" XDG_STATE_HOME="$t/state" CLAUDE_PLUGIN_ROOT="$plugin" JJ_CONFIG="$t/config.toml"
mkdir -p "$HOME"
printf 'user.name = "probe"\nuser.email = "probe@example.com"\n' >"$JJ_CONFIG"
"$hooks/build-bin.sh" jj-stale-descriptions
failed=0
check() { # label, then a test command
  local label=$1
  shift
  if "$@"; then echo "ok   $label"; else echo "FAIL $label"; failed=1; fi
}

repo="$t/repo"
jj git init "$repo" >/dev/null 2>&1
git init -q --bare "$t/remote.git"
jj -R "$repo" git remote add origin "$t/remote.git"
mkdir -p "$repo/workspaces"
for name in a b c; do
  jj -R "$repo" workspace add --name "$name" "$repo/workspaces/$name" >/dev/null 2>&1
done
a="$repo/workspaces/a" b="$repo/workspaces/b" c="$repo/workspaces/c"
change_of() { jj -R "$1" log --no-graph -r "${2:-@}" -T 'change_id.short(12)'; }

edit() { # session agent file
  printf 'edit\n' >>"$3"
  jq -nc --arg s "$1" --arg a "$2" --arg f "$3" \
    '{session_id: $s, tool_name: "Write", tool_input: {file_path: $f}} + (if $a == "main" then {} else {agent_id: $a} end)' |
    "$hooks/mark-writer.sh"
}
stop() { # dir session agent revset; prints the hook's stdout, fails unless it exits 0
  (cd "$1" && jq -nc --arg s "$2" --arg a "$3" \
    '{session_id: $s, stop_hook_active: false} + (if $a == "main" then {} else {agent_id: $a} end)' |
    "$hooks/stale-check.sh" "$4")
}
prompt() { jq -nc --arg s "$1" '{session_id: $s, prompt: "next"}' | "$hooks/prompt-note.sh"; }
push() { # cwd command; prints the hook's stdout
  jq -nc --arg c "$2" --arg cwd "$1" '{session_id: "p", cwd: $cwd, tool_name: "Bash", tool_input: {command: $c}}' |
    "$hooks/push-check.sh"
}
main='trunk()..@ ~ empty()'
sub='default@..@ ~ empty()'
says() { grep -qF -- "$2" <<<"$1"; }

# The end of a turn flags, never blocks.
out=$(stop "$a" fork main "$main")
check 'a session that never edited gets nothing' [ -z "$out" ]
edit writer main "$a/notes.txt"
mine=$(change_of "$a")
out=$(stop "$a" writer main "$main")
check 'an editor is flagged, to the user' says "$out" "\"systemMessage\":\"active-descriptions: changes edited this session have out-of-date descriptions ($mine)"
note=$(prompt writer)
check '...and to the agent on its next turn' says "$note" "additionalContext"
check '...naming the change and its workspace' says "$note" "In the workspace at $a:"
check '...once' [ -z "$(prompt writer)" ]
edit writer main "$a/notes.txt"
check 'the same set is not flagged again' [ -z "$(stop "$a" writer main "$main")" ]
(cd "$a" && jj describe -m notes >/dev/null 2>&1)
check 'nothing is flagged once it is described' [ -z "$(stop "$a" writer main "$main")" ]
edit writer main "$a/notes.txt"
check '...and it is flagged again when it goes out of date again' says "$(stop "$a" writer main "$main")" "$mine"
prompt writer >/dev/null
(cd "$a" && jj describe -m notes >/dev/null 2>&1 && jj new >/dev/null 2>&1)

edit sub-session sub1 "$c/sub.txt"
out=$(stop "$c" sub-session sub1 "$sub")
check 'a subagent is not shown to the user' [ -z "$out" ]
check '...but leaves its main thread a note' says "$(prompt sub-session)" "In the workspace at $c:"
check 'another subagent of that session is not flagged' [ -z "$(stop "$c" sub-session sub2 "$sub")" ]
check 'the main thread answers for its subagent' says "$(stop "$c" sub-session main "$main")" "systemMessage"
(cd "$c" && jj describe -m 'sub work' >/dev/null 2>&1 && jj new >/dev/null 2>&1)

# Drift: the shell ends up in another agent's workspace.
printf 'theirs\n' >"$b/theirs.txt"
(cd "$b" && jj status >/dev/null 2>&1)
theirs=$(change_of "$b")
edit drifter main "$c/stale.txt"
drifted=$(change_of "$c")
out=$(stop "$b" drifter main "$main")
check "a drifted shell is flagged for its own workspace" says "$out" "$drifted"
check "...not for the other agent's change" bash -c '! grep -qF -- "$1" <<<"$2"' _ "$theirs" "$out"

# Pushes.
(cd "$a" && jj bookmark create -r @- base >/dev/null 2>&1 && jj git push --bookmark base >/dev/null 2>&1)
printf 'feature\n' >"$a/feature.txt"
(cd "$a" && jj describe -m 'feature: first take' >/dev/null 2>&1 && jj bookmark create -r @ feature >/dev/null 2>&1)
printf 'more\n' >>"$a/feature.txt"
(cd "$a" && jj status >/dev/null 2>&1)
feature=$(change_of "$a")
denied() { says "$1" '"permissionDecision":"deny"' && says "$1" "$feature"; }
check 'a push of an out-of-date change is refused' denied "$(push "$a" 'jj git push --bookmark feature')"
check '...through cd and &&' denied "$(push "$t" "cd $a && jj git push -b feature")"
check '...through -R and --bookmark=' denied "$(push "$t" "jj -R $a git push --bookmark=feature")"
check '...and by default from the workspace' denied "$(push "$a" 'jj git push')"
check 'a dry run passes' [ -z "$(push "$a" 'jj git push -b feature --dry-run')" ]
check 'other commands pass' [ -z "$(push "$a" 'jj git fetch && git status')" ]
(cd "$a" && "$hooks/ack.sh" "$feature" >/dev/null)
check 'an acknowledged change passes' [ -z "$(push "$a" 'jj git push --bookmark feature')" ]
printf 'again\n' >>"$a/feature.txt"
(cd "$a" && jj status >/dev/null 2>&1)
check '...until its diff changes again' denied "$(push "$a" 'jj git push --bookmark feature')"
(cd "$a" && jj describe -m 'feature: done' >/dev/null 2>&1)
check 'a described change passes' [ -z "$(push "$a" 'jj git push --bookmark feature')" ]

exit "$failed"
