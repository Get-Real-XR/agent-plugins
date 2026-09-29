//! What agents are told about default@ being read-only.
//!
//! [`ReadOnlyNotice`] is added to an agent's context when it starts in
//! default@, so it knows the rule before anything is blocked. [`Denial`] is
//! the reason given for a blocked tool call. Either may be all the agent
//! knows about the guard, so both state the rule, why it exists, and how to
//! get a workspace. The allowed lists are generated from [`crate::commands`]
//! so they cannot drift from what the guard enforces.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::commands::{
    FORBIDDEN_FLAGS, GH_SUBCOMMANDS, GIT_LISTING_SUBCOMMANDS, GIT_SUBCOMMANDS, JJ_SUBCOMMANDS,
    READ_ONLY, SHELL_BUILTINS, UNTRACKED_WRITES, ZELLIJ_SUBCOMMANDS,
};

/// Context for an agent whose session or subagent starts in default@.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadOnlyNotice {
    pub default_root: PathBuf,
}

impl fmt::Display for ReadOnlyNotice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "jj-workspace-guard: default@ is read-only for agents.")?;
        writeln!(f)?;
        why_read_only(f, &self.default_root)?;
        writeln!(
            f,
            "Reading is fine here. Before changing anything (editing files, jj or git commands \
             that change state, builds), move to a jj workspace of your own:"
        )?;
        how_to_leave(f, &self.default_root)?;
        writeln!(f)?;
        write!(
            f,
            "Edits and non-read-only Bash commands in default@ are blocked, and the block \
             message lists what is allowed."
        )
    }
}

/// A blocked tool call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Denial {
    /// What in the call would write, as a sentence fragment.
    pub reason: String,
    /// The default workspace it would have written in.
    pub default_root: PathBuf,
}

impl fmt::Display for Denial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "jj-workspace-guard: default@ is read-only for agents, so this was blocked: {}.",
            self.reason
        )?;
        writeln!(f)?;
        why_read_only(f, &self.default_root)?;
        writeln!(f, "To make changes, work in a jj workspace of your own:")?;
        how_to_leave(f, &self.default_root)?;
        writeln!(f)?;
        writeln!(
            f,
            "Read, Grep, Glob and other tools that do not edit files are not affected. Allowed \
             in Bash in default@ without a workspace:"
        )?;
        writeln!(
            f,
            "  - {} {}",
            READ_ONLY.join(" "),
            UNTRACKED_WRITES.join(" ")
        )?;
        writeln!(f, "  - shell builtins: {}", SHELL_BUILTINS.join(" "))?;
        let flag_rules: Vec<String> = FORBIDDEN_FLAGS
            .iter()
            .map(|(command, flags)| format!("{command} (without {})", flags.join(" ")))
            .collect();
        writeln!(f, "  - {}", flag_rules.join(", "))?;
        writeln!(
            f,
            "  - sed as a filter only (`sed -n '1,50p' file`, `sed 's/a/b/g'`), uniq with at most \
             one file, command -v, env with no arguments"
        )?;
        writeln!(f, "  - jj {}", subcommand_list(JJ_SUBCOMMANDS))?;
        writeln!(
            f,
            "  - git {}, and the listing forms of {}",
            GIT_SUBCOMMANDS.join(", "),
            GIT_LISTING_SUBCOMMANDS.join(", ")
        )?;
        writeln!(
            f,
            "  - gh {} (api: GET only)",
            subcommand_list(GH_SUBCOMMANDS)
        )?;
        writeln!(f, "  - zellij {}", subcommand_list(ZELLIJ_SUBCOMMANDS))?;
        writeln!(
            f,
            "  - pipes, loops, command substitution and output redirects, as long as every \
             command is allowed and redirects write outside default@ (such as /dev/null or a \
             scratch directory)"
        )?;
        writeln!(f)?;
        write!(
            f,
            "Do not work around this guard, for example through another interpreter, a \
             different path to the same files, or another agent's workspace. If the task truly \
             needs a change in default@ itself, stop and ask the user."
        )
    }
}

fn why_read_only(f: &mut fmt::Formatter<'_>, root: &Path) -> fmt::Result {
    let root = root.display();
    writeln!(
        f,
        "{root} is the default jj workspace (default@). The user and other agents share its \
         working copy, so an edit or state-changing command here lands in someone else's \
         in-progress change."
    )?;
    writeln!(f)
}

fn how_to_leave(f: &mut fmt::Formatter<'_>, root: &Path) -> fmt::Result {
    let root = root.display();
    let workspace = format!("{root}/workspaces/<name>");
    writeln!(
        f,
        "  1. Run `jj workspace list` to find one already made for this task, or create one \
         with `mkdir -p {root}/workspaces && jj workspace add {workspace} -r <base revision>` \
         (allowed from default@, as is appending to the repo's .git/info/exclude)."
    )?;
    writeln!(
        f,
        "  2. `cd {workspace}`. The Bash tool keeps that directory for later calls, and \
         commands run there are not restricted."
    )?;
    writeln!(
        f,
        "  3. Edit and Write files by absolute path under {workspace}/."
    )
}

fn subcommand_list(subcommands: &[&[&str]]) -> String {
    subcommands
        .iter()
        .filter(|words| !words.is_empty())
        .map(|words| words.join(" "))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{Denial, ReadOnlyNotice};

    fn root() -> PathBuf {
        PathBuf::from("/home/dev/repos/project")
    }

    #[test]
    fn notice() {
        insta::assert_snapshot!(
            ReadOnlyNotice {
                default_root: root()
            }
            .to_string()
        );
    }

    #[test]
    fn denial() {
        let denial = Denial {
            reason: "`jj new` is not a read-only jj command".to_owned(),
            default_root: root(),
        };
        insta::assert_snapshot!(denial.to_string());
    }
}
