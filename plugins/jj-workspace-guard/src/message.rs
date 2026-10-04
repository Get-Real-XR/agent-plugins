//! What agents are told about where they may write.
//!
//! [`Notice`] is added to an agent's context when it starts somewhere
//! restricted, so it knows the rule before anything is blocked. [`Denial`] is
//! the reason given for a blocked tool call. Either may be all the agent
//! knows about the guard, so both state the rule, why it exists, and the way
//! forward. They summarize what is allowed; [`allowlist`] renders the full
//! lists from [`crate::commands`] into `ALLOWLIST.md`, which a test keeps
//! current, so the summary can point there without drifting from what the
//! guard enforces.

use std::ffi::{OsStr, OsString};
use std::fmt::{self, Write as _};
use std::path::{Path, PathBuf};

use crate::commands::{
    DISPOSABLE_WRITES, FORBIDDEN_FLAGS, GH_SUBCOMMANDS, GIT_LISTING_SUBCOMMANDS, GIT_SUBCOMMANDS,
    JJ_SUBCOMMANDS, OUTSIDE_TOOLS, READ_ONLY, SHELL_BUILTINS, UNTRACKED_WRITES, ZELLIJ_SUBCOMMANDS,
};
use crate::workspace::Zone;

/// Where jj-worktree-compat puts workspaces; the guard suggests the same
/// place.
const WORKSPACES_DIR_VAR: &str = "JJ_WORKTREE_COMPAT_DIR";
const DEFAULT_WORKSPACES_DIR: &str = ".claude/worktrees";

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
                write!(
                    f,
                    "Reading is fine here. Before changing anything (editing files, jj or git \
                     commands that change state, builds), "
                )?;
                how_to_leave(f, root)?;
                writeln!(f)?;
                write!(
                    f,
                    "Edits and Bash commands that are not read-only are blocked in default@."
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
                write!(f, "To make changes, ")?;
                how_to_leave(f, root)?;
                writeln!(f)?;
                writeln!(
                    f,
                    "Allowed in default@ without a workspace: Read, Grep, Glob and other tools \
                     that do not edit files; Bash that only reads (ls, cat, rg, jq, find or fd \
                     without exec, sed as a filter, …); jj, git, gh and zellij commands that \
                     only read (log, status, diff, show, …), plus `jj workspace add` and mkdir; \
                     {} on files in Claude's memory or a temporary folder with no repo in it, \
                     and rm on the folder of a workspace already forgotten; pipes and \
                     redirects that write outside default@.",
                    DISPOSABLE_WRITES.join(", ")
                )?;
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
                writeln!(f)?;
                writeln!(
                    f,
                    "Allowed outside a repo: Read, Grep, Glob and other tools that do not edit \
                     files; Bash that only reads (ls, cat, rg, jq, find or fd without exec, sed \
                     as a filter, …); `jj git init` and `jj git clone` (not `git init` or `git \
                     clone`); machine and service tools ({}); {} and writes in temporary \
                     folders and Claude's memory.",
                    OUTSIDE_TOOLS.join(", "),
                    DISPOSABLE_WRITES.join(", ")
                )?;
            }
        }
        if let Some(plugin_root) = plugin_root() {
            writeln!(
                f,
                "The full list is in {}.",
                plugin_root.join("ALLOWLIST.md").display()
            )?;
        }
        writeln!(f)?;
        let outside = !matches!(self.zone, Zone::Default(_));
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
    writeln!(
        f,
        "{} is the default jj workspace (default@). The user and other agents share its \
         working copy, so an edit or state-changing command here lands in someone else's \
         in-progress change.",
        root.display()
    )?;
    writeln!(f)
}

/// Continues a sentence that ends "…, " with how to move to a workspace.
fn how_to_leave(f: &mut fmt::Formatter<'_>, root: &Path) -> fmt::Result {
    let base = workspaces_dir(root, configured_workspaces_dir().as_deref());
    let base = base.display();
    writeln!(
        f,
        "work in a jj workspace of your own (`jj workspace list` shows any already made for \
         this task). Both commands are allowed from default@:"
    )?;
    writeln!(f, "  mkdir -p {base} && jj workspace add {base}/<name>")?;
    writeln!(f, "  cd {base}/<name>")?;
    writeln!(
        f,
        "The new workspace starts on the same parents as default@'s working copy (add `-r \
         <revision>` to start elsewhere, such as `trunk()`). The Bash tool keeps the directory \
         you `cd` into, nothing is restricted there, and Edit and Write take absolute paths \
         under it. To remove a workspace you made, run `jj workspace forget` inside it, then \
         `rm -rf` its folder from anywhere."
    )
}

fn how_to_enter(f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
        f,
        "`cd` into the repo the work belongs to (the Bash tool keeps that directory), clone it \
         with `jj git clone <url> <dir>`, or start one with `mkdir -p <dir> && cd <dir> && jj \
         git init` (both colocated by default; add `--no-colocate` for a repo only agents use, \
         such as a scratch repo for one-off tasks). Your instructions say where repos and \
         scratch work belong."
    )
}

/// The folder new workspaces of the repo whose default workspace is at
/// `root` go in, by the rule jj-worktree-compat uses: an absolute
/// `configured` folder gets a subfolder named after the repo, and a relative
/// one is taken from `root`.
pub fn workspaces_dir(root: &Path, configured: Option<&OsStr>) -> PathBuf {
    let configured = Path::new(
        configured
            .filter(|dir| !dir.is_empty())
            .unwrap_or_else(|| OsStr::new(DEFAULT_WORKSPACES_DIR)),
    );
    if configured.is_absolute() {
        configured.join(root.file_name().unwrap_or(root.as_os_str()))
    } else {
        root.join(configured)
    }
}

/// The configured workspaces folder. Tests ignore the environment, so the
/// snapshots do not depend on the machine running them.
fn configured_workspaces_dir() -> Option<OsString> {
    if cfg!(test) {
        None
    } else {
        std::env::var_os(WORKSPACES_DIR_VAR)
    }
}

/// The plugin's installed folder, which holds `ALLOWLIST.md`. Claude Code
/// sets `CLAUDE_PLUGIN_ROOT` for plugin hooks.
fn plugin_root() -> Option<PathBuf> {
    if cfg!(test) {
        Some(PathBuf::from(
            "/home/dev/.claude/plugins/cache/agent-plugins/jj-workspace-guard/1.0.0",
        ))
    } else {
        std::env::var_os("CLAUDE_PLUGIN_ROOT").map(PathBuf::from)
    }
}

/// The complete allowlists, as the Markdown kept in `ALLOWLIST.md`.
pub fn allowlist() -> String {
    let mut out = String::new();
    let flag_rules: Vec<String> = FORBIDDEN_FLAGS
        .iter()
        .map(|(command, flags)| format!("{command} (without {})", flags.join(" ")))
        .collect();
    let common = [
        format!("{} {}", READ_ONLY.join(" "), UNTRACKED_WRITES.join(" ")),
        format!("shell builtins: {}", SHELL_BUILTINS.join(" ")),
        flag_rules.join(", "),
        "sed as a filter only (`sed -n '1,50p' file`, `sed 's/a/b/g'`), uniq with at most one \
         file, command -v, env with no arguments"
            .to_owned(),
        format!(
            "{} on files in Claude's memory, or in a temporary folder with no repo in it (cp may \
             copy from anywhere)",
            DISPOSABLE_WRITES.join(" ")
        ),
        "rm, rmdir, and mv as a source, on the root folder of a workspace its repo no longer \
         lists, after `jj workspace forget`"
            .to_owned(),
    ];
    let section = |out: &mut String, title: &str, items: &[String]| {
        let _ = writeln!(out, "\n## {title}\n");
        for item in common.iter().chain(items) {
            let _ = writeln!(out, "- {item}");
        }
    };

    out.push_str(
        "# jj-workspace-guard allowlist\n\n\
         Generated from `src/commands.rs`; `UPDATE_ALLOWLIST=1 cargo nextest run` rewrites it, \
         and a test fails while it is out of date.\n\n\
         Read, Grep, Glob and other tools that do not edit files are never affected. Bash \
         commands are checked piece by piece, so pipes, loops and command substitution are \
         allowed when every command in them is.\n",
    );
    section(
        &mut out,
        "In default@ (the default jj workspace)",
        &[
            format!("jj {}", subcommand_list(JJ_SUBCOMMANDS)),
            format!(
                "git {}, and the listing forms of {}",
                GIT_SUBCOMMANDS.join(", "),
                GIT_LISTING_SUBCOMMANDS.join(", ")
            ),
            format!("gh {} (api: GET only)", subcommand_list(GH_SUBCOMMANDS)),
            format!("zellij {}", subcommand_list(ZELLIJ_SUBCOMMANDS)),
            "output redirects that write outside default@, such as /dev/null or a scratch \
             directory"
                .to_owned(),
            "Edit and Write only on the repo's git exclude file".to_owned(),
        ],
    );
    section(
        &mut out,
        "Outside any jj repo",
        &[
            format!(
                "jj git init, jj git clone, and jj {}",
                subcommand_list(JJ_SUBCOMMANDS)
            ),
            format!(
                "git {} (but not git init or git clone: use jj git init and jj git clone)",
                GIT_SUBCOMMANDS.join(", ")
            ),
            format!("machine and service tools: {}", OUTSIDE_TOOLS.join(", ")),
            "output redirects that write only to temporary folders (such as /tmp or /var/tmp) \
             or Claude's memory"
                .to_owned(),
            "Edit and Write only in temporary folders and Claude's memory".to_owned(),
        ],
    );
    out
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
    use std::ffi::OsStr;
    use std::path::{Path, PathBuf};

    use super::{Denial, Notice, allowlist, workspaces_dir};
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

    #[test]
    fn workspaces_go_where_jj_worktree_compat_puts_them() {
        let root = root();
        assert_eq!(workspaces_dir(&root, None), root.join(".claude/worktrees"));
        assert_eq!(
            workspaces_dir(&root, Some(OsStr::new(""))),
            root.join(".claude/worktrees")
        );
        assert_eq!(
            workspaces_dir(&root, Some(OsStr::new("workspaces"))),
            root.join("workspaces")
        );
        assert_eq!(
            workspaces_dir(&root, Some(OsStr::new("/home/dev/workspaces"))),
            Path::new("/home/dev/workspaces/project")
        );
    }

    #[test]
    fn allowlist_file_is_current() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("ALLOWLIST.md");
        let rendered = allowlist();
        if std::env::var_os("UPDATE_ALLOWLIST").is_some() {
            std::fs::write(&path, &rendered).expect("the crate directory is writable");
        }
        let committed = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            committed == rendered,
            "ALLOWLIST.md is out of date; rerun with UPDATE_ALLOWLIST=1"
        );
    }
}
