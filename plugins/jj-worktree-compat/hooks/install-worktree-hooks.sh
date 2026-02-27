#!/usr/bin/env bash
set -euo pipefail

# Install jj WorktreeCreate/WorktreeRemove hooks into ~/.claude/settings.json.
#
# Plugin hooks for these events don't fire (anthropics/claude-code#16288),
# so they must be installed into user-level settings where they work.
#
# Run via /install-jj-worktree skill. Counterpart: remove-worktree-hooks.sh.

settings_file="$HOME/.claude/settings.json"

: "${CLAUDE_PLUGIN_ROOT:?install-worktree-hooks: CLAUDE_PLUGIN_ROOT not set}"

create_cmd="$CLAUDE_PLUGIN_ROOT/hooks/jj-worktree-create.sh"
remove_cmd="$CLAUDE_PLUGIN_ROOT/hooks/jj-worktree-remove.sh"

# --- Check if already up to date --------------------------------------------

if [ -f "$settings_file" ]; then
  current=$(cat "$settings_file")

  existing_create=$(echo "$current" | jq -r \
    '[(.hooks.WorktreeCreate // [])[]] | map(select(._managed_by == "jj-worktree-compat")) | .[0].hooks[0].command // ""')
  existing_remove=$(echo "$current" | jq -r \
    '[(.hooks.WorktreeRemove // [])[]] | map(select(._managed_by == "jj-worktree-compat")) | .[0].hooks[0].command // ""')

  if [ "$existing_create" = "$create_cmd" ] && [ "$existing_remove" = "$remove_cmd" ]; then
    echo "jj worktree hooks already installed in $settings_file — nothing to do."
    exit 0
  fi
else
  current='{}'
fi

# --- Build and inject -------------------------------------------------------

desired_create=$(jq -n --arg cmd "$create_cmd" \
  '{_managed_by: "jj-worktree-compat", hooks: [{type: "command", command: $cmd, timeout: 30}]}')

desired_remove=$(jq -n --arg cmd "$remove_cmd" \
  '{_managed_by: "jj-worktree-compat", hooks: [{type: "command", command: $cmd, timeout: 30}]}')

updated=$(echo "$current" | jq \
  --argjson create "$desired_create" \
  --argjson remove "$desired_remove" \
  '
  .hooks //= {} |
  .hooks.WorktreeCreate = ([(.hooks.WorktreeCreate // [])[]] | map(select(._managed_by != "jj-worktree-compat"))) |
  .hooks.WorktreeRemove = ([(.hooks.WorktreeRemove // [])[]] | map(select(._managed_by != "jj-worktree-compat"))) |
  .hooks.WorktreeCreate += [$create] |
  .hooks.WorktreeRemove += [$remove]
  ')

echo "$updated" > "$settings_file"
echo "Installed jj worktree hooks in $settings_file."
echo "This is a workaround for anthropics/claude-code#16288 (plugin WorktreeCreate/Remove hooks don't fire)."
echo "Restart the session for hooks to take effect."
echo "Run /remove-jj-worktree to uninstall."
