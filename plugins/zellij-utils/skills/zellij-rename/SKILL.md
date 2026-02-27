---
name: zellij-rename
aliases:
  - zr
description: >
  Rename the current zellij pane, tab, or session with a contextually
  derived label. Usage: /zellij-rename {pane|tab|session}
user_invocable: true
---

# Zellij Rename

Rename a zellij pane, tab, or session using context from the current
session to produce a short, informative label.

## Arguments

The first argument is the target: `pane`, `tab`, or `session`.
If omitted, default to `tab`.

## Label derivation

Derive the label from whatever is most salient in the current session:

- **tab**: project name or repo name + current task (e.g. `agent-plugins: worktree hooks`)
- **pane**: current activity or file focus (e.g. `editing hooks.json`)
- **session**: project or workspace identity (e.g. `agent-plugins`)

Labels must be short (under 30 chars), lowercase, no special characters
beyond `:`, `-`, and spaces.

## Execution

Run a single bash command:

```
zellij action rename-{target} "{label}"
```

No confirmation needed. Just run it.
