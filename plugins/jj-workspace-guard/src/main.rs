//! Claude Code hook that keeps agents from writing in the default jj
//! workspace.
//!
//! Several agents, and the user, may share one repo. Each agent gets its own
//! jj workspace for changes; the default workspace stays read-only so no one
//! edits a working copy someone else is in the middle of.
//!
//! - On `SessionStart` and `SubagentStart` in default@, the agent is told the
//!   rule up front, so it moves to a workspace before anything is blocked.
//! - On `PreToolUse`, file-editing tools aimed inside default@ are denied, as
//!   are Bash commands run there that are not known to be read-only. The
//!   reason tells the agent how to proceed. Any other call gets no output,
//!   leaving the decision to the normal permission flow; the guard never
//!   answers `allow`.

mod commands;
mod message;
mod shell;
mod workspace;

use std::io::{Read, Write};
use std::path::PathBuf;

use anyhow::Context;
use serde::Deserialize;
use serde_json::json;

use crate::message::{Denial, Notice};
use crate::workspace::Zone;

/// The parts of the hook input the guard reads, by event.
#[derive(Debug, Deserialize)]
#[serde(tag = "hook_event_name")]
enum HookInput {
    PreToolUse(ToolCall),
    SessionStart(Start),
    SubagentStart(Start),
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
struct ToolCall {
    tool_name: String,
    #[serde(default)]
    tool_input: ToolInput,
    cwd: PathBuf,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ToolInput {
    command: Option<String>,
    file_path: Option<PathBuf>,
    notebook_path: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct Start {
    cwd: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let mut raw = String::new();
    std::io::stdin()
        .read_to_string(&mut raw)
        .context("read the hook input")?;
    let input: HookInput = serde_json::from_str(&raw).context("parse the hook input")?;
    let context = |event: &str, notice: Notice| json!({ "hookSpecificOutput": { "hookEventName": event, "additionalContext": notice.to_string() } });
    let output = match input {
        HookInput::PreToolUse(call) => check(&call).map(|denial| {
            json!({
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "deny",
                    "permissionDecisionReason": denial.to_string(),
                }
            })
        }),
        HookInput::SessionStart(start) => {
            notice(&start).map(|notice| context("SessionStart", notice))
        }
        HookInput::SubagentStart(start) => {
            notice(&start).map(|notice| context("SubagentStart", notice))
        }
        HookInput::Other => None,
    };
    if let Some(output) = output {
        let mut stdout = std::io::stdout().lock();
        serde_json::to_writer(&mut stdout, &output).context("write the hook output")?;
        stdout.flush().context("flush the hook output")?;
    }
    Ok(())
}

fn notice(start: &Start) -> Option<Notice> {
    match workspace::zone(&start.cwd) {
        Zone::Free => None,
        zone => Some(Notice { zone }),
    }
}

fn check(input: &ToolCall) -> Option<Denial> {
    match input.tool_name.as_str() {
        "Bash" => shell::check(input.tool_input.command.as_deref()?, &input.cwd).err(),
        "Edit" | "MultiEdit" | "Write" | "NotebookEdit" => {
            let target = input
                .tool_input
                .file_path
                .as_ref()
                .or(input.tool_input.notebook_path.as_ref())?;
            let target = workspace::absolutize(&input.cwd, target);
            match workspace::write_zone(&target) {
                Zone::Free => None,
                zone => Some(Denial {
                    reason: format!("{} would modify {}", input.tool_name, target.display()),
                    zone,
                }),
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use serde_json::json;

    use super::{HookInput, ToolCall, check, notice};
    use crate::workspace::Zone;
    use crate::workspace::tests::{Layout, layout};

    fn input(layout: &Layout, tool_name: &str, tool_input: serde_json::Value) -> ToolCall {
        let input = serde_json::from_value(json!({
            "session_id": "test",
            "hook_event_name": "PreToolUse",
            "cwd": layout.root,
            "tool_name": tool_name,
            "tool_input": tool_input,
        }));
        match input.unwrap() {
            HookInput::PreToolUse(call) => call,
            other => panic!("parsed as {other:?}"),
        }
    }

    fn start(event: &str, cwd: &std::path::Path) -> HookInput {
        serde_json::from_value(json!({
            "session_id": "test",
            "hook_event_name": event,
            "source": "startup",
            "agent_type": "general-purpose",
            "cwd": cwd,
        }))
        .unwrap()
    }

    #[rstest]
    fn agents_starting_in_default_are_told_it_is_read_only(layout: Layout) {
        for event in ["SessionStart", "SubagentStart"] {
            let (HookInput::SessionStart(started) | HookInput::SubagentStart(started)) =
                start(event, &layout.root)
            else {
                panic!("{event} did not parse as a start event");
            };
            assert_eq!(notice(&started).unwrap().zone, Zone::Default(layout.root.clone()));
        }
    }

    #[rstest]
    fn agents_in_an_added_workspace_are_told_nothing(layout: Layout) {
        let HookInput::SessionStart(started) = start("SessionStart", &layout.added) else {
            panic!("SessionStart did not parse");
        };
        assert!(notice(&started).is_none());
    }

    #[rstest]
    fn agents_outside_any_repo_are_told_how_to_get_into_one(layout: Layout) {
        let HookInput::SessionStart(started) = start("SessionStart", &layout.outside) else {
            panic!("SessionStart did not parse");
        };
        assert_eq!(notice(&started).unwrap().zone, Zone::Outside);
    }

    #[rstest]
    fn edits_outside_any_repo_are_denied_except_scratch(layout: Layout) {
        let denial = check(&input(&layout, "Write", json!({ "file_path": "/srv/guard-probe/notes.md" })))
            .unwrap();
        assert_eq!(denial.zone, Zone::Outside);
        let scratch = json!({ "file_path": layout.outside.join("notes.md") });
        assert!(check(&input(&layout, "Write", scratch)).is_none());
    }

    #[rstest]
    fn other_events_are_ignored(layout: Layout) {
        assert!(matches!(start("Stop", &layout.root), HookInput::Other));
    }

    #[rstest]
    fn edits_in_default_are_denied(layout: Layout) {
        let file = layout.root.join("src/main.rs");
        for tool in ["Edit", "Write", "MultiEdit"] {
            let denial = check(&input(&layout, tool, json!({ "file_path": file }))).unwrap();
            assert_eq!(denial.zone, Zone::Default(layout.root.clone()));
        }
        let notebook = json!({ "notebook_path": layout.root.join("analysis.ipynb") });
        assert!(check(&input(&layout, "NotebookEdit", notebook)).is_some());
    }

    #[rstest]
    fn relative_edit_paths_resolve_against_cwd(layout: Layout) {
        let tool_input = json!({ "file_path": "src/main.rs" });
        assert!(check(&input(&layout, "Write", tool_input)).is_some());
    }

    #[rstest]
    fn edits_elsewhere_are_allowed(layout: Layout) {
        let exclude = layout.root.join(".git/info/exclude");
        for file in [
            layout.added.join("src/main.rs"),
            layout.outside.join("notes.md"),
            exclude,
        ] {
            assert!(check(&input(&layout, "Write", json!({ "file_path": file }))).is_none());
        }
    }

    #[rstest]
    fn bash_is_checked_and_other_tools_pass(layout: Layout) {
        assert!(check(&input(&layout, "Bash", json!({ "command": "jj new" }))).is_some());
        assert!(check(&input(&layout, "Bash", json!({ "command": "jj st" }))).is_none());
        let read = json!({ "file_path": layout.root.join("src/main.rs") });
        assert!(check(&input(&layout, "Read", read)).is_none());
    }
}
