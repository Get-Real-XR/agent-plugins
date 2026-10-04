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

All four work together but can be installed independently. [jj-workspace-guard](#jj-workspace-guard) is opt-in on top of them: it changes what agents may do in your main checkout.

## Plugins

### active-descriptions

Keeps your jj change descriptions in sync with your actual changes.

| | |
|---|---|
| **Install** | `claude plugin install active-descriptions@agent-plugins` |
| **Skill** | `/describe` (user-invocable) |
| **Requires** | jj, Rust toolchain (cargo) |

Blocks session exit until every in-flight change has an up-to-date description. Only an agent that edited files in the workspace (with Edit, Write or NotebookEdit) is held to it, so a `/fork` or another session sharing the workspace can stop; a main session also answers for its subagents' edits. Each set of stale changes blocks once: an agent that tries to stop again without describing them is let go rather than looped. Edits made only through Bash do not count. When the agent detects drift, it runs `/describe` — reading the diff and conversation history, drafting a Conventional Commits description, and applying it via `jj describe`. Also gates Bash commands on being inside a jj repository, letting through commands that make or clone one; with jj-workspace-guard enabled it defers to the guard's finer rules.

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

Creates a jj workspace sharing the same parents as your current working copy; cleans up automatically on removal, after snapshotting it so no edit is lost. Drop-in replacement for Claude Code's built-in git worktrees. Like a git worktree made from `HEAD`, the new workspace does not contain your in-progress change; building it on top of that change instead would freeze the change for you wherever other workspaces' working copies are immutable (`working_copies()` in `immutable_heads()`). `plugins/jj-worktree-compat/tests/create-keeps-caller-mutable.sh` checks this.

Workspaces go under `.claude/worktrees/` by default. Set `JJ_WORKTREE_COMPAT_DIR` (for example in the `env` block of Claude Code's settings) to put them elsewhere; an absolute path gets a subfolder per repo, and a relative path is taken from the default workspace's root, even when the caller is in an added workspace.

Current Claude Code refuses a worktree that git resolves to an enclosing checkout. A jj workspace in a colocated repo, or anywhere under a git-managed home directory, has no `.git` of its own, so `EnterWorktree` and isolated subagents fail there until jj can give each workspace its own Git worktree. Create workspaces with `jj workspace add` and `cd` into them instead.

Versions before 0.3.0 installed these hooks into `~/.claude/settings.json` as a workaround for a Claude Code bug. The plugin now registers them itself; run `/jj-worktree-hook-workaround-remove` once to delete the old entries.

### jj-workspace-guard

Makes the default jj workspace (`default@`) read-only for agents, so parallel agents never edit a working copy you or another agent is in the middle of.

| | |
|---|---|
| **Install** | `claude plugin install jj-workspace-guard@agent-plugins` |
| **Requires** | jj, Rust toolchain (cargo) |

A `PreToolUse` hook blocks `Edit`, `Write` and `NotebookEdit` on files inside `default@`, and Bash commands run there unless every command in them is on a read-only allowlist (`ls`, `rg`, `jq`, `jj log`, `git diff`, `gh pr view`, …). The bash is parsed, so pipes, loops, substitutions and redirects are checked piece by piece, and a leading `cd` into another workspace is followed. The block message tells the agent to create its own workspace with `jj workspace add`, `cd` into it, and work there, where nothing is restricted. It suggests the folder jj-worktree-compat uses (`JJ_WORKTREE_COMPAT_DIR`, default `.claude/worktrees`), summarizes what is allowed, and points to the full list in [`plugins/jj-workspace-guard/ALLOWLIST.md`](plugins/jj-workspace-guard/ALLOWLIST.md), which `jj-workspace-guard allowlist` also prints and a test keeps current.

It guards against mistakes, not adversaries: once an agent's shell is in another workspace, the guard does not inspect what its commands touch.

Outside any jj repo, it keeps project work out of unversioned folders: agents may read and search, use machine and service tools (`aws`, `chezmoi`, `claude`, `gh`, `gpuq`, `kubectl`, `op-agent`, `ssh`, `zellij`), and make or clone a jj repo (`jj git init`, `jj git clone`; plain `git init` and `git clone` are refused). Writes outside a repo go only to temporary folders and Claude's memory. The message points the agent into a repo, and agents starting outside one are told the same up front.

Repos that are not projects, such as a dotfiles source or a scratch repo, can opt out with `jj config set --repo jj-workspace-guard.enabled false`, run in that repo. Claude Code's memory directories (`~/.claude/projects/*/memory/`) are always writable.

`rm`, `rmdir`, `unlink`, `mv`, `cp` and `touch` are allowed anywhere when everything they change is a disposable file: one in a Claude Code memory directory, or in a temporary folder with no jj or git repo in or under it. `cp` may copy from anywhere into such a place. So agents can delete or rename a memory, or clean up their own temporary files, without a workspace. `rm` and `rmdir` (and `mv` of a source) may also remove the root folder of a workspace that its repo no longer lists: an agent finishing with a workspace runs `jj workspace forget` inside it, then removes the folder from anywhere. Registered workspaces, and paths inside any workspace, stay protected.

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
| jj-workspace-guard | required | required |

active-descriptions and jj-workspace-guard are Rust. Each installed version builds its binary once, the first time a session needs it, into a cargo target folder shared by all versions (`$XDG_CACHE_HOME/agent-plugins/target`, by default `~/.cache/agent-plugins/target`), and copies it to the plugin's `bin/`; hooks run that binary directly. A new version recompiles only the plugin crate and whatever dependencies changed.

Install jj: [jj-vcs.github.io/jj/latest/install-and-setup](https://jj-vcs.github.io/jj/latest/install-and-setup/)
Install Rust: [rustup.rs](https://rustup.rs/)
