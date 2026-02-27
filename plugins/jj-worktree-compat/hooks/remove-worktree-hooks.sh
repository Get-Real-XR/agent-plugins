#!/usr/bin/env bash
set -euo pipefail

# Remove jj-worktree-compat managed hooks from ~/.claude/settings.json.
# Counterpart to install-worktree-hooks.sh. Run via /remove-jj-worktree skill.

settings_file="$HOME/.claude/settings.json"

if [ ! -f "$settings_file" ]; then
  echo "No ~/.claude/settings.json found — nothing to clean up."
  exit 0
fi

updated=$(jq '
  # Remove managed entries
  (.hooks.WorktreeCreate // []) |= map(select(._managed_by != "jj-worktree-compat")) |
  (.hooks.WorktreeRemove // []) |= map(select(._managed_by != "jj-worktree-compat")) |
  # Prune empty arrays
  if .hooks.WorktreeCreate == [] then del(.hooks.WorktreeCreate) else . end |
  if .hooks.WorktreeRemove == [] then del(.hooks.WorktreeRemove) else . end |
  # Prune empty hooks object
  if .hooks == {} then del(.hooks) else . end
' "$settings_file")

echo "$updated" > "$settings_file"
echo "Removed jj-worktree-compat hooks from $settings_file."
