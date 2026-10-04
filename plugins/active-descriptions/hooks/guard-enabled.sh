# Sourced by the hooks: whether jj-workspace-guard is enabled in user or
# project settings. When it is, it owns the rules for where agents may work.
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
