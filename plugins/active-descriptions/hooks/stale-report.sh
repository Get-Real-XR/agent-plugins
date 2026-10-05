# Sourced by the hooks: finding out-of-date descriptions.

source "$(dirname "${BASH_SOURCE[0]}")/guard-enabled.sh"
source "$(dirname "${BASH_SOURCE[0]}")/plugin-bin.sh"

# Prints the workspace roots that agent $2 edited files in, as mark-writer.sh
# recorded them in session state folder $1. The main thread ("main") also
# answers for its subagents' edits.
edited_roots() {
  awk -F'\t' -v agent="$2" '
    (agent == "main" || $1 == agent) && !seen[$2]++ { print $2 }' "$1/writers" 2>/dev/null
}

# Prints, for each workspace root on stdin, the changes in revset $1 there
# whose descriptions are out of date, under a line naming the workspace.
# Changes another workspace's working copy is built on belong to whoever
# works there, and are left out. A guarded default workspace is skipped:
# agents cannot describe anything there.
stale_report() {
  local revset=$1 bin root found nl=$'\n' report=""
  bin=$(plugin_bin jj-stale-descriptions) || return 0
  while IFS= read -r root; do
    [ -d "$root/.jj" ] || continue
    # Only the default workspace holds `.jj/repo` as a directory; added
    # workspaces hold a file pointing to it.
    if [ -d "$root/.jj/repo" ] && guard_enabled; then
      continue
    fi
    found=$(cd "$root" && "$bin" "($revset) ~ ::(working_copies() ~ @)" 2>/dev/null) || continue
    [ -n "$found" ] && report="${report:+$report$nl}In the workspace at $root:$nl$found"
  done
  printf '%s' "$report"
}

# The change ids a report flags, one per line, sorted.
flagged_changes() {
  grep -oE 'Stale description: change [k-z]+' <<<"$1" | sed 's/.* //' | sort -u
}
