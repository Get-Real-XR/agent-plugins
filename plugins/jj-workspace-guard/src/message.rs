//! What agents are told about where they may write.
//!
//! [`Notice`] is added to an agent's context when it starts somewhere
//! restricted, so it knows the rule before anything is blocked. [`Denial`] is
//! the reason given for a blocked tool call. Either may be all the agent
//! knows about the guard, so both state the rule, why it exists, and the way
//! forward. The allowed lists are generated from [`crate::commands`] so they
//! cannot drift from what the guard enforces.

use std::fmt;
use std::path::Path;

use crate::commands::{
    DISPOSABLE_WRITES, FORBIDDEN_FLAGS, GH_SUBCOMMANDS, GIT_LISTING_SUBCOMMANDS, GIT_SUBCOMMANDS,
    JJ_SUBCOMMANDS, OUTSIDE_TOOLS, READ_ONLY, SHELL_BUILTINS, UNTRACKED_WRITES, ZELLIJ_SUBCOMMANDS,
};
use crate::workspace::Zone;

/// Context for an agent whose session or subagent starts in a restricted
/// zone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub zone: Zone,
}

impl fmt::Display for Notice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.zone {
            Zone::Default(root) => {
                writeln!(f, "jj-workspace-guard: default@ is read-only for agents.")?;
                writeln!(f)?;
                why_read_only(f, root)?;
                writeln!(
                    f,
                    "Reading is fine here. Before changing anything (editing files, jj or git \
                     commands that change state, builds), move to a jj workspace of your own:"
                )?;
                how_to_leave(f, root)?;
                writeln!(f)?;
                write!(
                    f,
                    "Edits and non-read-only Bash commands in default@ are blocked, and the block \
                     message lists what is allowed."
                )
            }
            Zone::Outside | Zone::Free => {
                writeln!(f, "jj-workspace-guard: you are outside any jj repo.")?;
                writeln!(f)?;
                write!(
                    f,
                    "Reading, searching and machine tools ({}) are fine here. Project work \
                     happens in a jj repo; ",
                    OUTSIDE_TOOLS.join(", ")
                )?;
                how_to_enter(f)?;
                write!(
                    f,
                    " Outside a repo, edits and writes are allowed only in temporary folders and \
                     Claude's memory."
                )
            }
        }
    }
}

/// A blocked tool call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Denial {
    /// What in the call would write, as a sentence fragment.
    pub reason: String,
    /// Where it would have written: a default workspace, or outside any repo.
    pub zone: Zone,
}

impl fmt::Display for Denial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let outside = !matches!(self.zone, Zone::Default(_));
        match &self.zone {
            Zone::Default(root) => {
                writeln!(
                    f,
                    "jj-workspace-guard: default@ is read-only for agents, so this was blocked: \
                     {}.",
                    self.reason
                )?;
                writeln!(f)?;
                why_read_only(f, root)?;
                writeln!(f, "To make changes, work in a jj workspace of your own:")?;
                how_to_leave(f, root)?;
            }
            Zone::Outside | Zone::Free => {
                writeln!(
                    f,
                    "jj-workspace-guard: you are outside any jj repo, so this was blocked: {}.",
                    self.reason
                )?;
                writeln!(f)?;
                write!(
                    f,
                    "Project work happens in a jj repo, where every change is recorded; "
                )?;
                how_to_enter(f)?;
                writeln!(f)?;
            }
        }
        writeln!(f)?;
        writeln!(
            f,
            "Read, Grep, Glob and other tools that do not edit files are not affected. Allowed \
             in Bash {}:",
            if outside {
                "outside a repo"
            } else {
                "in default@ without a workspace"
            }
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
        writeln!(
            f,
            "  - {} on files in Claude's memory, or in a temporary folder with no repo in it \
             (cp may copy from anywhere)",
            DISPOSABLE_WRITES.join(" ")
        )?;
        if outside {
            writeln!(
                f,
                "  - jj git init, jj git clone, and jj {}",
                subcommand_list(JJ_SUBCOMMANDS)
            )?;
            writeln!(
                f,
                "  - git {} (but not git init or git clone: use jj git init and jj git clone)",
                GIT_SUBCOMMANDS.join(", ")
            )?;
            writeln!(
                f,
                "  - machine and service tools: {}",
                OUTSIDE_TOOLS.join(", ")
            )?;
            writeln!(
                f,
                "  - pipes, loops, command substitution and output redirects, as long as every \
                 command is allowed and redirects write only to temporary folders (such as /tmp \
                 or /var/tmp) or Claude's memory"
            )?;
            writeln!(
                f,
                "Edit and Write outside a repo are allowed only in temporary folders and Claude's \
                 memory."
            )?;
        } else {
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
        }
        writeln!(f)?;
        write!(
            f,
            "Do not work around this guard, for example through another interpreter, a \
             different path to the same files, or another agent's workspace. If the task truly \
             needs {}, stop and ask the user.",
            if outside {
                "a file outside any repo"
            } else {
                "a change in default@ itself"
            }
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

fn how_to_enter(f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
        f,
        "`cd` into the repo the work belongs to (the Bash tool keeps that directory), clone it \
         with `jj git clone --colocate <url> <dir>`, or start one with `mkdir -p <dir> && cd \
         <dir> && jj git init` (colocated by default; add `--no-colocate` for a repo only \
         agents use, such as a scratch repo for one-off tasks). Your instructions say where \
         repos and scratch work belong."
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

    use super::{Denial, Notice};
    use crate::workspace::Zone;

    fn root() -> PathBuf {
        PathBuf::from("/home/dev/repos/project")
    }

    #[test]
    fn notice() {
        insta::assert_snapshot!(
            Notice {
                zone: Zone::Default(root())
            }
            .to_string()
        );
    }

    #[test]
    fn notice_outside() {
        insta::assert_snapshot!(
            Notice {
                zone: Zone::Outside
            }
            .to_string()
        );
    }

    #[test]
    fn denial() {
        let denial = Denial {
            reason: "`jj new` is not a read-only jj command".to_owned(),
            zone: Zone::Default(root()),
        };
        insta::assert_snapshot!(denial.to_string());
    }

    #[test]
    fn denial_outside() {
        let denial = Denial {
            reason: "`cargo` is not on the read-only allowlist".to_owned(),
            zone: Zone::Outside,
        };
        insta::assert_snapshot!(denial.to_string());
    }
}
