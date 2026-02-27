---
name: jj-worktree-hook-workaround-remove
description: >
  Remove jj workspace hooks from ~/.claude/settings.json.
  Reverses /jj-worktree-hook-workaround-install.
user-invocable: true
allowed-tools:
  - Bash: */remove-worktree-hooks.sh
---

Run the removal script and show the user its output.
The `hooks/` directory is two levels up from this skill's base directory:

```bash
"{base_dir}/../../hooks/remove-worktree-hooks.sh"
```

Replace `{base_dir}` with the absolute path from the "Base directory"
header injected at the top of this prompt.
