#!/bin/bash

# Runs jj-workspace-guard as a Codex hook. Codex sends hook input in Claude
# Code's format, so the same binary serves both; see the README for the
# ~/.codex/hooks.json entries.
#
# Codex runs this from a checkout that updates in place, such as the
# agent-plugins marketplace clone, not from a per-version plugin cache. So
# rebuild whenever plugin.json, whose version every release bumps, is newer
# than the installed binary.

root=$(cd "$(dirname "$0")/.." && pwd)
bin="$root/bin/jj-workspace-guard"
if [ ! -x "$bin" ] || [ "$root/.claude-plugin/plugin.json" -nt "$bin" ]; then
  rm -f "$bin"
  "$root/hooks/build-bin.sh" jj-workspace-guard </dev/null >/dev/null 2>&1
fi
CLAUDE_PLUGIN_ROOT="$root" exec "$bin"
