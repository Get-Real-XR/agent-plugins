#!/bin/bash

# Stop/SubagentStop hook: block stopping while a change in the revset given as
# $1 has a stale description.
#
# With jj-workspace-guard enabled, the default workspace is read-only for
# agents: one working there cannot describe anything, and the changes there
# are someone else's. Skip in that case instead of demanding the impossible.

revset=$1

guard_enabled() {
  local settings
  for settings in "$HOME/.claude/settings.json" \
    "${CLAUDE_PROJECT_DIR:-.}/.claude/settings.json" \
    "${CLAUDE_PROJECT_DIR:-.}/.claude/settings.local.json"; do
    [ -f "$settings" ] &&
      jq -e '.enabledPlugins["jj-workspace-guard@agent-plugins"] == true' "$settings" >/dev/null 2>&1 &&
      return 0
  done
  return 1
}

root=$(jj root 2>/dev/null) || exit 0
# Only the default workspace holds `.jj/repo` as a directory; added
# workspaces hold a file pointing to it.
if [ -d "$root/.jj/repo" ] && guard_enabled; then
  exit 0
fi

msg=$(cargo run -r --manifest-path "${CLAUDE_PLUGIN_ROOT}/Cargo.toml" -- "$revset" 2>/dev/null) || exit 0
[ -n "$msg" ] || exit 0
echo "$msg" >&2
exit 2
