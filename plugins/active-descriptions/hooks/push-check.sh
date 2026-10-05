#!/bin/bash

# PreToolUse(Bash) hook: refuse a `jj git push` that would publish a change
# whose description is out of date. jj itself refuses changes with no
# description; this adds the ones whose diff moved on since they were
# described. A push is where a description starts to matter to others, and
# unlike the end of a turn, the work is done by then.
#
# The command is split at `&&`, `||`, `;`, `|` and newlines, following `cd`.
# For each `jj git push`, the changes it publishes are the ancestors of the
# bookmarks or revisions it names (jj's default: bookmarks between the
# remote and @) that no remote bookmark contains. Anything the hook cannot
# work out, such as a revset jj rejects, lets the push through.

source "$(dirname "$0")/plugin-bin.sh"

input=$(cat)
case "$input" in *"git push"*) ;; *) exit 0 ;; esac
command=$(jq -r '.tool_input.command // ""' <<<"$input")
dir=$(jq -r '.cwd // ""' <<<"$input")
[ -n "$dir" ] || dir=$PWD

resolve() { # path relative to $dir
  case "$1" in
    /*) printf '%s' "$1" ;;
    "~" | "~/"*) printf '%s' "$HOME${1#\~}" ;;
    *) printf '%s' "$dir/$1" ;;
  esac
}
bookmark_revset() {
  case "$1" in
    *:*) printf 'bookmarks(%s)' "${1%%:*}:\"${1#*:}\"" ;;
    *) printf 'bookmarks(exact:"%s")' "$1" ;;
  esac
}

bin=""
reason=""
nl=$'\n'
while IFS= read -r segment; do
  set -f
  read -ra words <<<"$segment"
  set +f
  [ ${#words[@]} -gt 0 ] || continue
  for i in "${!words[@]}"; do
    words[i]=${words[i]//\"/}
    words[i]=${words[i]//\'/}
  done
  if [ "${words[0]}" = cd ]; then
    dir=$(resolve "${words[1]:-$HOME}")
    continue
  fi
  [ "${words[0]}" = jj ] || continue

  repo=$dir
  i=1
  while [ "$i" -lt ${#words[@]} ]; do
    case "${words[i]}" in
      -R | --repository) repo=$(resolve "${words[i + 1]:-}"); i=$((i + 2)) ;;
      --repository=*) repo=$(resolve "${words[i]#*=}"); i=$((i + 1)) ;;
      --config | --config-file | --at-op | --at-operation | --color) i=$((i + 2)) ;;
      -*) i=$((i + 1)) ;;
      *) break ;;
    esac
  done
  [ "${words[i]:-}" = git ] && [ "${words[i + 1]:-}" = push ] || continue
  i=$((i + 2))

  heads=()
  explicit=false
  dry_run=false
  while [ "$i" -lt ${#words[@]} ]; do
    word=${words[i]}
    value=${words[i + 1]:-}
    case "$word" in
      -b | --bookmark) heads+=("$(bookmark_revset "$value")"); explicit=true; i=$((i + 2)) ;;
      --bookmark=*) heads+=("$(bookmark_revset "${word#*=}")"); explicit=true; i=$((i + 1)) ;;
      -r | --revisions | -c | --change) heads+=("($value)"); explicit=true; i=$((i + 2)) ;;
      --revisions=* | --change=*) heads+=("(${word#*=})"); explicit=true; i=$((i + 1)) ;;
      --named) heads+=("(${value#*=})"); explicit=true; i=$((i + 2)) ;;
      --named=*) heads+=("(${word#*=*=})"); explicit=true; i=$((i + 1)) ;;
      --all | --tracked) heads+=("bookmarks()"); explicit=true; i=$((i + 1)) ;;
      --deleted) explicit=true; i=$((i + 1)) ;;
      --dry-run) dry_run=true; i=$((i + 1)) ;;
      --remote) i=$((i + 2)) ;;
      *) i=$((i + 1)) ;;
    esac
  done
  $dry_run && continue
  $explicit || heads=("bookmarks() & (remote_bookmarks()..@)")
  [ ${#heads[@]} -gt 0 ] || continue

  revset=$(IFS='|'; printf '((::(%s)) ~ ::remote_bookmarks()) ~ empty()' "${heads[*]}")
  [ -n "$bin" ] || bin=$(plugin_bin jj-stale-descriptions) || exit 0
  found=$(cd "$repo" 2>/dev/null && "$bin" "$revset" 2>/dev/null) || continue
  [ -n "$found" ] && reason="${reason:+$reason$nl}$found"
done < <(awk '{ gsub(/&&|\|\||[;|]/, "\n"); print }' <<<"$command")

[ -n "$reason" ] || exit 0
ack="$(cd "$(dirname "$0")" && pwd)/ack.sh"
jq -nc --arg found "$reason" --arg ack "$ack" '{hookSpecificOutput: {
  hookEventName: "PreToolUse",
  permissionDecision: "deny",
  permissionDecisionReason: (
    "active-descriptions: this push would publish changes whose descriptions are out of date:\n" +
    $found +
    "\nUpdate each description with the describe skill. If one still fits after a formatting-only or generated change, acknowledge it instead, from its workspace: \"" +
    $ack + "\" <change-id>. Then push again.")}}'
