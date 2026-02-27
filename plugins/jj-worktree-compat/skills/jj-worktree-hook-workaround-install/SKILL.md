---
name: jj-worktree-hook-workaround-install
description: >
  Install jj workspace hooks into ~/.claude/settings.json.
  Workaround for anthropics/claude-code#16288.
user-invocable: true
allowed-tools:
  - Bash: */install-worktree-hooks.sh
---

Run the install script and show the user its output.
The `hooks/` directory is two levels up from this skill's base directory:

```bash
"{base_dir}/../../hooks/install-worktree-hooks.sh"
```

Replace `{base_dir}` with the absolute path from the "Base directory"
header injected at the top of this prompt.
