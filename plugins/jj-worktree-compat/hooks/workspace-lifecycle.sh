#!/usr/bin/env bash
# Lifecycle of jj workspaces that agents add by hand (`jj workspace add`),
# which Claude Code's WorktreeRemove never sees. Without it, finished
# workspaces pile up, each with its own build output.
#
#   track          PostToolUse(Bash): remember workspaces this session adds.
#   session-end    SessionEnd: remove this session's idle workspaces.
#   sweep REPO [--apply] [--forgotten]
#                  List a repo's idle workspaces unused for a day, and
#                  folders of workspaces jj has forgotten; remove them with
#                  --apply (forgotten folders also need --forgotten).
#
# A workspace is idle when, after a fresh snapshot, its working-copy change is
# empty and undescribed: everything done there is in commits that outlive the
# workspace. A workspace stays while something else works in it: a Claude or
# Codex session whose transcript names it as its cwd within the last hour, or
# a process whose current directory is inside it.
set -u

state_dir="${XDG_STATE_HOME:-$HOME/.local/state}/jj-worktree-compat/sessions"

jjq() { jj --no-pager --color never "$@"; }

canonical() { (cd "$1" 2>/dev/null && pwd -P); }

# Whether $1 is the root of an added workspace (whose `.jj/repo` is a file
# pointing at the store) that its repo still lists.
registered() {
  local root
  root=$(canonical "$1") || return 1
  [ -f "$root/.jj/repo" ] || return 1
  jjq -R "$root" --ignore-working-copy workspace list -T 'root ++ "\n"' 2>/dev/null |
    while IFS= read -r listed; do
      [ -n "$listed" ] && [ "$(canonical "$listed")" = "$root" ] && echo yes && break
    done | grep -q yes
}

# Whether the workspace's working-copy change, as jj last recorded it, is
# empty and undescribed.
recorded_idle() {
  [ "$(jjq -R "$1" --ignore-working-copy log --no-graph -r @ \
    -T 'if(empty, if(description, "", "idle"))' 2>/dev/null)" = idle ]
}

# The same after snapshotting the workspace, so recent edits count.
idle() {
  (cd "$1" && jjq status >/dev/null 2>&1) && recorded_idle "$1"
}

# Whether a jj command changed the workspace's working copy in the last day.
recent() {
  [ -n "$(find "$1/.jj/working_copy" -mmin -1440 2>/dev/null | head -1)" ]
}

# Whether another session or a process works in $1. $2 is this session's id,
# whose own transcript does not count.
in_use() {
  local root=$1 own=${2:-} transcript
  while IFS= read -r transcript; do
    if [ -n "$own" ]; then
      case "$transcript" in */"$own".jsonl | */"$own"/*) continue ;; esac
    fi
    grep -qF "\"cwd\":\"$root" "$transcript" 2>/dev/null && return 0
  done < <(find "$HOME/.claude/projects" "$HOME/.codex/sessions" -name '*.jsonl' -mmin -60 2>/dev/null)
  if [ -d /proc/self ]; then
    local proc
    for proc in /proc/[0-9]*; do
      case "$(readlink "$proc/cwd" 2>/dev/null)" in "$root" | "$root"/*) return 0 ;; esac
    done
  elif command -v lsof >/dev/null; then
    lsof -a -d cwd -Fn 2>/dev/null | grep -qE "^n$root(/|\$)" && return 0
  fi
  return 1
}

# Forgets the workspace at $1 if jj still lists it, then deletes its folder.
remove() {
  if registered "$1"; then
    (cd "$1" && jjq workspace forget >/dev/null 2>&1) || return 1
  fi
  rm -rf "$1"
}

track() {
  local input session cwd command word path root
  input=$(cat)
  case "$input" in *"workspace add"*) ;; *) return 0 ;; esac
  session=$(jq -r '.session_id // ""' <<<"$input")
  cwd=$(jq -r '.cwd // ""' <<<"$input")
  command=$(jq -r '.tool_input.command // ""' <<<"$input")
  [ -n "$session" ] || return 0
  mkdir -p "$state_dir"
  # Any word of the command that names a workspace root created in the last
  # ten minutes; without one, any such workspace of the repo at cwd.
  # Split the command into words without expanding globs in it.
  local found=()
  set -f
  for word in $command; do
    word=${word//\"/}
    word=${word//\'/}
    word=${word%;}
    word=${word/#\$\{HOME\}/$HOME}
    word=${word/#\$HOME/$HOME}
    word=${word/#\~/$HOME}
    case "$word" in "" | -*) continue ;; /*) path=$word ;; *) path=$cwd/$word ;; esac
    [ -f "$path/.jj/repo" ] && [ -n "$(find "$path/.jj/repo" -mmin -10 2>/dev/null)" ] &&
      found+=("$(canonical "$path")")
  done
  set +f
  if [ ${#found[@]} -eq 0 ] && [ -n "$cwd" ]; then
    while IFS= read -r root; do
      [ -n "$root" ] && [ -f "$root/.jj/repo" ] &&
        [ -n "$(find "$root/.jj/repo" -mmin -2 2>/dev/null)" ] && found+=("$(canonical "$root")")
    done < <(cd "$cwd" 2>/dev/null && jjq --ignore-working-copy workspace list -T 'root ++ "\n"' 2>/dev/null)
  fi
  for root in ${found[@]+"${found[@]}"}; do
    grep -qxF "$root" "$state_dir/$session" 2>/dev/null || printf '%s\n' "$root" >>"$state_dir/$session"
  done
}

# Claude Code may cancel SessionEnd hooks as it exits, so the cleanup runs
# detached in its own session.
session_end() {
  local session
  session=$(jq -r '.session_id // ""')
  [ -n "$session" ] && [ -f "$state_dir/$session" ] || return 0
  perl -e 'use POSIX; POSIX::setsid(); exec @ARGV' "$0" cleanup-session "$session" \
    </dev/null >/dev/null 2>&1 &
}

cleanup_session() {
  local session=$1 file="$state_dir/$1" root
  [ -f "$file" ] || return 0
  : >"$file.keep"
  while IFS= read -r root; do
    [ -d "$root" ] || continue
    if ! registered "$root" || { idle "$root" && ! in_use "$root" "$session"; }; then
      remove "$root" || printf '%s\n' "$root" >>"$file.keep"
    else
      printf '%s\n' "$root" >>"$file.keep"
    fi
  done <"$file"
  # Workspaces with work in progress stay tracked, in case the session resumes.
  if [ -s "$file.keep" ]; then mv "$file.keep" "$file"; else rm -f "$file" "$file.keep"; fi
}

sweep() {
  local repo="" apply=false forgotten=false arg
  for arg in "$@"; do
    case "$arg" in --apply) apply=true ;; --forgotten) forgotten=true ;; *) repo=$arg ;; esac
  done
  [ -n "$repo" ] || { echo "usage: $0 sweep REPO [--apply] [--forgotten]" >&2; return 2; }
  local store default_root name root listed=""
  store=$(canonical "$(jjq -R "$repo" --ignore-working-copy root 2>/dev/null)")/.jj/repo || return 1
  [ -f "$store" ] && store=$(cd "$(dirname "$store")" && canonical "$(cat repo)")
  default_root=$(dirname "$(dirname "$store")")

  while IFS=$'\t' read -r name root; do
    [ -n "$root" ] && [ "$name" != default ] || continue
    root=$(canonical "$root") || { echo "missing    $name (listed, but its folder is gone)"; continue; }
    listed="$listed$root"$'\n'
    # Listing judges from jj's last record: a snapshot would itself make
    # the workspace recent. Removing snapshots first and checks again.
    if recent "$root"; then
      echo "recent     $root"
    elif in_use "$root"; then
      echo "in use     $root"
    elif ! $apply; then
      recorded_idle "$root" && echo "idle       $root" || echo "has work   $root"
    elif idle "$root"; then
      remove "$root" && echo "removed    $root" || echo "FAILED     $root"
    else
      echo "has work   $root"
    fi
  done < <(jjq -R "$default_root" --ignore-working-copy workspace list -T 'name ++ "\t" ++ root ++ "\n"' 2>/dev/null)

  local parents=("$default_root/workspaces" "$default_root/.claude/worktrees") parent folder
  case "${JJ_WORKTREE_COMPAT_DIR:-}" in
    /*) parents+=("$JJ_WORKTREE_COMPAT_DIR/$(basename "$default_root")") ;;
    ?*) parents+=("$default_root/$JJ_WORKTREE_COMPAT_DIR") ;;
  esac
  for parent in "${parents[@]}"; do
    for folder in "$parent"/*; do
      [ -f "$folder/.jj/repo" ] || continue
      folder=$(canonical "$folder")
      case "$listed" in *"$folder"$'\n'*) continue ;; esac
      [ "$(cd "$folder/.jj" && canonical "$(cat repo)")" = "$store" ] || continue
      if $apply && $forgotten && ! in_use "$folder"; then
        rm -rf "$folder" && echo "removed    $folder (forgotten)"
      else
        echo "forgotten  $folder ($(du -sh "$folder" 2>/dev/null | cut -f1); edits since it was forgotten are not recorded)"
      fi
    done
  done
}

case "${1:-}" in
  track) track ;;
  session-end) session_end ;;
  cleanup-session) cleanup_session "$2" ;;
  sweep) shift; sweep "$@" ;;
  *) echo "usage: $0 track | session-end | sweep REPO [--apply] [--forgotten]" >&2; exit 2 ;;
esac
