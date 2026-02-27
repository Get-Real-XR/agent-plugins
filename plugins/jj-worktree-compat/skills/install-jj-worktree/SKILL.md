---
name: install-jj-worktree
description: >
  Install jj workspace hooks into .claude/settings.local.json.
  Workaround for anthropics/claude-code#16288.
user_invocable: true
---

Run the install script and show the user its output:

```bash
"${CLAUDE_PLUGIN_ROOT}/hooks/install-worktree-hooks.sh"
```
