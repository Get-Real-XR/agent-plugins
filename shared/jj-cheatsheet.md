# Jujutsu (jj) Cheatsheet

## Mental Model

- **Change vs. Commit.** A Change is a logical unit of work that persists
  across rewrites (stable letter-based **change ID**). A Commit is a specific
  snapshot (mutable hex **commit ID**). One Change can have many Commits as it
  evolves. This is jj's conceptual hinge — understand it early.
- **Working copy is auto-snapshotted.** No staging area, no "dirty" state.
  Every command boundary captures the working copy as a commit. New files are
  tracked by default (controlled by `.gitignore` and `snapshot.auto-track`).
- **Automatic descendant rebasing.** Editing an earlier change automatically
  rebases all descendants. Mutations are repository-wide by default.
- **Bookmarks auto-follow rewrites.** Unlike Git refs, bookmarks move with
  their commits through rebases and rewrites.
- **Conflicts are data, not errors.** You can commit conflicted files and
  resolve later. Conflicts are narrowly scoped to the specific diff that
  changed.
- **Divergent changes.** When one change ID points to multiple visible commits
  (e.g., from concurrent edits), jj marks them as divergent (`??` in logs).
  Resolve by abandoning duplicates or merging.
- **Operations are logged and undoable.** Every command is an operation in a
  DAG with full repo state snapshots. `jj op log` to inspect, `jj op undo` to
  rewind, `jj op restore` to jump to any prior state.

## First-Time Setup

```bash
jj config set --user user.name "Your Name"
jj config set --user user.email "you@example.com"
jj config set --user ui.editor "your-editor"      # vim, code --wait, etc.
```

## Core Commands

| Action                          | jj command                                          | git equivalent              |
|---------------------------------|-----------------------------------------------------|-----------------------------|
| Show working copy status        | `jj status`                                         | `git status`                |
| Show log                        | `jj log`                                            | `git log --graph`           |
| Show diff                       | `jj diff`                                           | `git diff`                  |
| Show change contents            | `jj show REV`                                       | `git show`                  |
| Create new empty change         | `jj new`                                            | (no equivalent)             |
| Move to next change             | `jj next`                                           | (no equivalent)             |
| Move to previous change         | `jj prev`                                           | (no equivalent)             |
| Snapshot + describe + new       | `jj commit --message "msg"`                         | `git commit -am "msg"`      |
| Set description                 | `jj describe --message "msg"`                       | `git commit --amend` (msg)  |
| Squash into parent              | `jj squash`                                         | `git rebase -i` (squash)    |
| Absorb hunks into ancestors     | `jj absorb`                                         | (no equivalent)             |
| Split current change            | `jj split`                                          | `git add -p && git commit`  |
| Edit an earlier change          | `jj edit CHANGE`                                    | `git rebase -i`             |
| Duplicate a change              | `jj duplicate REV`                                  | (no equivalent — safe copy) |
| Rebase single rev               | `jj rebase --revision REV --destination DEST`       | `git rebase`                |
| Rebase branch                   | `jj rebase --branch REV --destination DEST`         | `git rebase`                |
| Abandon (discard) a change      | `jj abandon`                                        | (no equivalent)             |
| Revert a change (inverse commit)| `jj revert --revision REV`                          | `git revert`                |
| Restore file from another rev   | `jj restore --from REV path`                        | `git checkout REV -- path`  |
| Resolve conflicts interactively | `jj resolve`                                        | (manual editing)            |
| Undo last operation             | `jj op undo`                                        | (no equivalent)             |
| Restore to a prior op state     | `jj op restore OP`                                  | (no equivalent)             |
| Revert a specific operation     | `jj op revert OP`                                   | (no equivalent)             |
| Find bug by bisection           | `jj bisect`                                         | `git bisect`                |

## Bookmark Operations

```bash
jj bookmark set NAME                           # create or move to current change
jj bookmark set NAME --revision REV            # create or move to specific revision
jj bookmark delete NAME                        # delete locally
jj bookmark list                               # list all bookmarks
jj bookmark track NAME@origin                  # track a remote bookmark
jj bookmark untrack NAME@origin                # stop tracking
```

Key differences from Git:
- No "current branch" concept — bookmarks are just pointers.
- Bookmarks auto-follow rewrites (rebase, squash, etc.).
- Conflicted bookmarks (shown with `??`) can arise from concurrent ops.

## Git Interop

```bash
jj git clone URL [DIR]                         # clone (--colocated for git+jj)
jj git push                                    # push all tracked bookmarks
jj git push --bookmark NAME                    # push specific bookmark
jj git push --change CHANGE                    # auto-create push-<id> bookmark and push
jj git fetch                                   # fetch from all remotes
jj git import                                  # import git refs (usually automatic)
jj git export                                  # export jj refs to git
```

Colocated repos (`.git/` + `.jj/` together) allow using Git and jj
interchangeably on the same repo. Safe for gradual adoption.

## Revset Syntax

### Symbols and Operators

| Symbol / Operator   | Meaning                                    |
|---------------------|--------------------------------------------|
| `@`                 | current working-copy change                |
| `@-`                | parent of working copy                     |
| `@--`               | grandparent                                |
| `x-`                | parent of x                                |
| `x+`                | children of x                              |
| `x::y`              | DAG range from x to y (inclusive)          |
| `::x`               | ancestors of x                             |
| `x::`               | descendants of x                           |
| `x..y`              | y's ancestors minus x's ancestors          |
| `~x`                | negation (complement)                      |
| `x & y`             | intersection                               |
| `x \| y`            | union                                      |
| `x ~ y`             | difference (x but not y)                   |

### Functions

| Function              | Meaning                                          |
|-----------------------|--------------------------------------------------|
| `trunk()`             | default remote's primary bookmark (configurable) |
| `bookmarks()`         | all local bookmarks                              |
| `remote_bookmarks()`  | all remote bookmarks                             |
| `heads(x)`            | revs in x with no children in x                  |
| `roots(x)`            | revs in x with no parents in x                   |
| `ancestors(x, depth)` | ancestors of x, optionally bounded by depth      |
| `descendants(x)`      | descendants of x                                 |
| `parents(x)`          | parents of x                                     |
| `children(x)`         | children of x                                    |
| `fork_point(x)`       | common ancestor(s)                               |
| `description(pat)`    | changes matching description                     |
| `author(pat)`         | changes by author                                |
| `mine()`              | changes by configured user                       |
| `empty()`             | changes with no diff                             |
| `conflict()`          | changes with conflicts                           |
| `divergent()`         | changes with multiple visible commits            |
| `immutable()`         | changes protected from modification              |
| `mutable()`           | changes that can be modified                     |
| `present(x)`          | x if it exists, empty set otherwise (no error)   |
| `latest(x, count)`    | most recent N revisions from x                   |
| `files(fileset)`      | changes modifying files matching fileset          |

Pattern syntax: `exact:"..."`, `glob:"..."`, `substring:"..."`, `regex:"..."`.

## Filesets

Filter files in operations like `jj diff`, `jj split`, `jj log`:

```bash
jj diff '~Cargo.lock'                          # exclude lock files
jj split 'glob:"**/*.rs"'                      # split only .rs files
jj log --stat 'src ~ glob:"**/*_test.rs"'      # src minus test files
```

Operators: `~x` (not), `x ~ y` (difference), `x & y` (and), `x | y` (or).

## GitHub Workflow

```bash
# Start work
jj new trunk()                                 # new change from main
# ... edit files (auto-saved) ...
jj commit --message "feat: add feature"        # snapshot + describe
jj bookmark set my-feature                  # create/move bookmark
jj git push --bookmark my-feature              # push to remote

# Update after review
jj edit CHANGE                                 # go back to the change
# ... make edits ...
jj bookmark set my-feature                    # advance bookmark to current change
jj git push --bookmark my-feature              # force-push update

# Squash fixup into the right commit
jj squash                                      # squash into parent
# or: jj absorb                                # auto-distribute hunks to ancestors

# Rebase in-progress work onto latest trunk
jj git fetch                                   # fetch team updates
jj rebase --branch @ --destination trunk()     # rebase current stack onto trunk

# After PR merges upstream
jj git fetch                                   # fetch updated main
jj abandon CHANGE                              # discard local copy (it's in trunk now)
jj bookmark delete my-feature                  # clean up local bookmark
jj new trunk()                                 # start fresh work
```

### Landing directly on main

When a change doesn't need a PR, advance `main` and push:

```bash
jj bookmark set main --revision @-             # advance main to the finished change
jj git push                                  # pushes all tracked bookmarks
```

### Feature branches via `--change`

`jj git push --change @-` auto-creates a `push-<id>` bookmark — convenient
for quick PRs but leaves bookmarks you must clean up later:

```bash
jj bookmark delete push-CHANGEID            # delete locally
jj git push --deleted                        # delete on remote
```

Prefer named bookmarks (`jj bookmark set`) for anything long-lived.

## Official Documentation

Links may change — if any are broken, go to <https://docs.jj-vcs.dev/latest/>
and navigate from there. When the cheatsheet and docs are insufficient to meet
the user's need, use web search.

**Getting Started**
- [Installation and setup](https://docs.jj-vcs.dev/latest/install-and-setup/)
- [Tutorial and bird's eye view](https://docs.jj-vcs.dev/latest/tutorial/)
- [Working with GitHub](https://docs.jj-vcs.dev/latest/github/)
- [FAQ](https://docs.jj-vcs.dev/latest/FAQ/)

**Concepts**
- [Working copy](https://docs.jj-vcs.dev/latest/working-copy/)
- [Bookmarks](https://docs.jj-vcs.dev/latest/bookmarks/)
- [Conflicts](https://docs.jj-vcs.dev/latest/conflicts/)
- [Operation log](https://docs.jj-vcs.dev/latest/operation-log/)
- [Glossary](https://docs.jj-vcs.dev/latest/glossary/)

**Guides**
- [CLI options for specifying revisions](https://docs.jj-vcs.dev/latest/guides/cli-revision-options/)
- [Divergent changes](https://docs.jj-vcs.dev/latest/guides/divergence/)
- [Multiple remotes](https://docs.jj-vcs.dev/latest/guides/multiple-remotes/)

**Reference**
- [CLI reference](https://docs.jj-vcs.dev/latest/cli-reference/)
- [Settings](https://docs.jj-vcs.dev/latest/config/)
- [Revset language](https://docs.jj-vcs.dev/latest/revsets/)
- [Fileset language](https://docs.jj-vcs.dev/latest/filesets/)
- [Templating language](https://docs.jj-vcs.dev/latest/templates/)

**Git Migration**
- [Git comparison](https://docs.jj-vcs.dev/latest/git-comparison/)
- [Git command table](https://docs.jj-vcs.dev/latest/git-command-table/)
- [Jujutsu for Git experts](https://docs.jj-vcs.dev/latest/git-experts/)
- [Git compatibility](https://docs.jj-vcs.dev/latest/git-compatibility/)
