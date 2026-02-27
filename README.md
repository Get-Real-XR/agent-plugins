# agent-plugins

Claude Code plugins for Diderot. Your agent writes change descriptions, enforces conventional commits, teaches you [jj](https://jj-vcs.github.io/jj/) as you work, and isolates worktrees via jj workspaces.

## Quick Start

Add the marketplace:

```sh
claude plugin marketplace add get-real-xr/agent-plugins
```

Install plugins:

```sh
claude plugin install active-descriptions@agent-plugins
claude plugin install conventional-commits@agent-plugins
claude plugin install jj-tutor@agent-plugins
claude plugin install jj-worktree-compat@agent-plugins
```

All four work together but can be installed independently.

## Plugins

### active-descriptions

Keeps your jj change descriptions in sync with your actual changes.

| | |
|---|---|
| **Install** | `claude plugin install active-descriptions@agent-plugins` |
| **Skill** | `/describe` (user-invocable) |
| **Requires** | jj, Rust toolchain (cargo) |

Blocks session exit until every in-flight change has an up-to-date description. When the agent detects drift, it runs `/describe` — reading the diff and conversation history, drafting a Conventional Commits description, and applying it via `jj describe`. Also gates Bash commands on being inside a jj repository.

Invoke `/describe` manually at any time to co-author a description mid-session.

### conventional-commits

Formats all change descriptions to the [Conventional Commits](https://www.conventionalcommits.org/) v1.0.0 spec.

| | |
|---|---|
| **Install** | `claude plugin install conventional-commits@agent-plugins` |
| **Skill** | agent context (loaded automatically) |
| **Requires** | — |

Every description the agent writes follows the `type[scope][!]: description` structure. Works standalone or as the formatting layer for `/describe`.

### jj-tutor

Teaches you jj at your own pace.

| | |
|---|---|
| **Install** | `claude plugin install jj-tutor@agent-plugins` |
| **Skill** | agent context (loaded automatically) |
| **Requires** | jj |

Asks your VCS experience level on first use (five tiers, from "brand new to the terminal" through "pre-existing jj user"), then adapts explanation depth, Git comparisons, confirmation thresholds, and vocabulary introduction to match. Tracks per-command and per-concept familiarity across sessions and backs off as you demonstrate comfort.

### jj-worktree-compat

Routes Claude Code worktree isolation through jj workspaces.

| | |
|---|---|
| **Install** | `claude plugin install jj-worktree-compat@agent-plugins` |
| **Requires** | jj |

Creates a jj workspace sharing the same parents as your current working copy; cleans up automatically on removal. Drop-in replacement for Claude Code's built-in git worktrees.

## How the jj plugins work together

- **active-descriptions** enforces that every session ends with up-to-date descriptions, using **conventional-commits** for formatting. `/describe` ties them together.
- **jj-worktree-compat** handles workspace lifecycle independently, so agent isolation works natively with jj.
- **jj-tutor** adapts to your level regardless of what else is installed — no coupling to the other plugins.

## Prerequisites

| Plugin | jj | Rust toolchain |
|---|---|---|
| active-descriptions | required | required |
| conventional-commits | — | — |
| jj-tutor | required | — |
| jj-worktree-compat | required | — |

Install jj: [jj-vcs.github.io/jj/latest/install-and-setup](https://jj-vcs.github.io/jj/latest/install-and-setup/)
Install Rust: [rustup.rs](https://rustup.rs/)
