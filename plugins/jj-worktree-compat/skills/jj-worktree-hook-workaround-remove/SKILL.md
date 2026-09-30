---
name: jj-worktree-hook-workaround-remove
description: >
  Remove the jj workspace hooks that plugin versions before 0.3.0 installed
  into ~/.claude/settings.json. The plugin now registers these hooks itself.
user-invocable: true
allowed-tools: "Bash(*/remove-worktree-hooks.sh)"
---

Run the removal script and show the user its output.
The `hooks/` directory is two levels up from this skill's base directory:

```bash
"{base_dir}/../../hooks/remove-worktree-hooks.sh"
```

Replace `{base_dir}` with the absolute path from the "Base directory"
header injected at the top of this prompt.
