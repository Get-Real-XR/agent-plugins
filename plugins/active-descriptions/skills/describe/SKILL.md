---
name: describe
description: >
  Collaborative jj change description workflow. Use when writing, drafting,
  or reviewing a jj change description, or when stale descriptions are
  flagged by the staleness hooks. Analyzes the diff and conversation
  history, then applies the description via jj describe.
user-invocable: true
---

# Active Change Description Workflow

Write change descriptions as a **joint artifact**, co-authored by human and
agent. Each contributes what they're uniquely positioned to know, at the
moment when that knowledge is freshest.

Audience priority: **reviewer > teammates/clients > future investigators > tooling**.

Format: [Conventional Commits](conventional-commits.md).
VCS reference: [jj cheatsheet](jj-cheatsheet.md).

## The commit checkpoint

The commit step is not just documentation. It is a natural pause for
reflection on the work itself. Legitimate outcomes include:

- A well-described change lands.
- "We're not done yet" — the description process reveals gaps.
- "This should be structured differently" — split, reorder, or rethink.
- "I want to take a different approach" — diverge with a new change.

Treat description-writing as a moment to evaluate the work, not just
record it.

## Stale description trigger

This plugin includes a Stop hook that blocks the session from ending when
changes have diverged from their description. When the stop hook fires,
treat each flagged change as a describe target:

1. Run `jj diff -r <change>` to get the full current diff.
2. Enter the standard workflow below (Phase 1–2) for that change.

Handle multiple flagged changes sequentially.

## Phase 1: Internal analysis

Before engaging the human, analyze internally:

- **The diff.** Ground truth. Run `jj diff` (or `jj diff -r <change>`) and
  account for everything that actually changed, not just what was intended.
  The description may not contradict or omit from the diff.
- **The conversation history.** The full decision trail — every directive,
  correction, and change of direction. This is a first-class input, not
  crumbs.
- **Mechanical analysis.** Type classification, scope detection, atomicity
  assessment, completeness checks (tests touched? docs updated? error
  paths handled?), what the code used to do vs. what it does now.

This gives you a rich but incomplete picture. The gaps are what the
human knows but never said aloud.

## Phase 2: Write and apply

Write the full description using everything available: the diff, the
conversation history, and your mechanical analysis. Apply it directly
via `jj describe`.

**Match depth to the change.** Not every commit warrants a lengthy body.
Reason about proportionality for each specific change in its specific
context rather than following fixed categories.

## What makes a good description

**Title:** The effect, not the implementation. What changed for the
user or system. The "how" goes in the body.

- BAD: `fix: add mutex to guard database handle`
- GOOD: `fix: prevent database corruption during simultaneous sign-ups`

**Body:** Inverted pyramid. Most important information first. The reader
should be able to stop at any depth and have gotten maximum value.

**Content to include** (scaled to complexity — simple changes need only a
one-liner):

- **Motivation.** The human's actual goal. Most important after the title.
- **Decision provenance.** Which decisions were human directives vs. agent
  choices. A future reader needs to know what to question and who to ask.
- **Breaking changes.** What breaks and how to adapt.
- **Alternatives considered.** What was rejected and why.
- **Non-obvious findings.** Surprising behavior, constraints, corrected
  assumptions discovered during implementation.
- **Testing boundary.** What was verified (by the agent and by the human
  outside the session), how, and what wasn't tested.
- **External references.** Issue trackers, audit findings, related
  discussions — non-obvious ones the reader couldn't trivially find.
- **New dependencies.** Flag and justify.
- **Cross-references.** `Fixes #1234`, related changes. Summarize linked
  issues.
- **Searchable artifacts.** Error messages, component names, task IDs.

**Content to leave out:**

- What's obvious from the diff.
- Invariants that belong in code comments or automated checks.
- Ephemeral discussion that belongs in PR comments.
- Transient artifacts (preview URLs, expiring build links).

**Self-contained.** The next reader may have zero access to the original
conversation. The description + diff must be sufficient on their own.

**No noise.** Every sentence must survive: "Would this help someone
reading this at 2 AM while debugging production?"
