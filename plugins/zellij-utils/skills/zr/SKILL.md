---
name: zr
description: >
  Rename the current zellij pane, tab, or session with a contextually
  derived label. Usage: /zr {t|p|s}
user-invocable: true
allowed-tools:
  - Bash: zellij action rename-*
  - Bash: zellij action query-tab-names
  - Bash: zellij action dump-layout
---

# Zellij Rename

Rename a zellij pane, tab, or session using context from the current
session to produce a short, informative label.

## Arguments

- `/zr t` — rename **tab**
- `/zr p` — rename **pane**
- `/zr s` — rename **session**

If omitted, default to `t` (tab).

## Gathering context

Before choosing a label, gather context appropriate to the target:

- **session** (`s`): run `zellij action query-tab-names` to see all tab
  names in the session. Derive a label that summarizes the session's
  overall purpose from its tabs.
- **tab** (`t`): use the project name, current branch/change, and what
  the session is actively working on. Optionally run
  `zellij action dump-layout` for pane details (commands, cwd) if the
  conversation context is thin.
- **pane** (`p`): use the immediate activity — file being edited, test
  being run, etc. Pure conversation context, no extra commands needed.

## Label rules

- Under 30 characters
- Lowercase
- Only `:`, `-`, and spaces beyond alphanumerics

## Execution

Run a single bash command — no confirmation needed:

```
zellij action rename-tab "{label}"
zellij action rename-pane "{label}"
zellij action rename-session "{label}"
```
