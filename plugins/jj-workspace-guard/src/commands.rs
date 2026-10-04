//! The commands an agent may run in the default workspace.
//!
//! Each allowed command is read-only: it writes no files and changes no jj or
//! git state, whatever its arguments, except where a rule below forbids the
//! arguments that would. The rules are tables so the block message can list
//! exactly what is allowed.

use std::path::Path;

use crate::workspace;

/// A command-line argument: its literal value, or `None` when it contains an
/// expansion the guard cannot evaluate.
pub type Arg = Option<String>;

/// Commands that are read-only whatever their arguments.
pub const READ_ONLY: &[&str] = &[
    "b2sum",
    "base64",
    "basename",
    "cat",
    "cksum",
    "cmp",
    "column",
    "comm",
    "cut",
    "date",
    "df",
    "diff",
    "dirname",
    "du",
    "echo",
    "egrep",
    "false",
    "fgrep",
    "fold",
    "free",
    "grep",
    "head",
    "hexdump",
    "hostname",
    "id",
    "jq",
    "join",
    "ls",
    "md5sum",
    "nl",
    "nproc",
    "od",
    "paste",
    "pgrep",
    "printenv",
    "printf",
    "ps",
    "pwd",
    "readlink",
    "realpath",
    "rev",
    "seq",
    "sha1sum",
    "sha256sum",
    "sha512sum",
    "stat",
    "strings",
    "tac",
    "tail",
    "test",
    "[",
    "tr",
    "true",
    "type",
    "uname",
    "uptime",
    "wc",
    "which",
    "whoami",
];

/// Commands that write, but nothing jj tracks: `mkdir` makes only empty
/// directories, and `jj workspace add` needs the new workspace's parent to
/// exist.
pub const UNTRACKED_WRITES: &[&str] = &["mkdir"];

/// Builtins that change only the shell running the command.
pub const SHELL_BUILTINS: &[&str] = &[
    ":", "cd", "declare", "exit", "export", "local", "read", "set", "shift", "unset", "wait",
];

/// Commands that are read-only unless given one of the listed flags.
///
/// A `--long` flag also matches `--long=value`; a `-x` flag matches any
/// short-flag cluster containing `x`; any other single-dash word (`find`'s
/// `-delete`) matches exactly.
pub const FORBIDDEN_FLAGS: &[(&str, &[&str])] = &[
    ("fd", &["-x", "-X", "--exec", "--exec-batch"]),
    ("file", &["-C", "--compile"]),
    (
        "find",
        &[
            "-delete", "-exec", "-execdir", "-fls", "-fprint", "-fprint0", "-fprintf", "-ok",
            "-okdir",
        ],
    ),
    ("rg", &["--pre"]),
    ("sort", &["-o", "--output"]),
    ("tree", &["-o"]),
];

/// Read-only jj subcommands, matched against the leading positional arguments
/// after [`JJ_ALIASES`] are expanded.
pub const JJ_SUBCOMMANDS: &[&[&str]] = &[
    &[],
    &["log"],
    &["status"],
    &["diff"],
    &["show"],
    &["evolog"],
    &["interdiff"],
    &["root"],
    &["help"],
    &["version"],
    &["op", "log"],
    &["op", "show"],
    &["op", "diff"],
    &["file", "list"],
    &["file", "show"],
    &["file", "annotate"],
    &["file", "search"],
    &["bookmark", "list"],
    &["tag", "list"],
    &["config", "list"],
    &["config", "get"],
    &["config", "path"],
    &["git", "remote", "list"],
    &["git", "root"],
    &["workspace", "list"],
    &["workspace", "root"],
    // Creating a workspace is how an agent gets out of default@, and it
    // writes only the new workspace.
    &["workspace", "add"],
];

/// jj's built-in aliases, by position: `jj b l` is `jj bookmark list`.
const JJ_ALIASES: &[(usize, &str, &str)] =
    &[(0, "b", "bookmark"), (0, "st", "status"), (1, "l", "list")];

/// jj's global flags that take a value.
const JJ_VALUE_FLAGS: &[&str] = &[
    "-R",
    "--repository",
    "--at-op",
    "--at-operation",
    "--color",
    "--config",
    "--config-file",
    "--config-toml",
];

/// git subcommands that are read-only in every form except `--output`.
pub const GIT_SUBCOMMANDS: &[&str] = &[
    "annotate",
    "blame",
    "cat-file",
    "check-attr",
    "check-ignore",
    "count-objects",
    "describe",
    "diff",
    "diff-files",
    "diff-index",
    "diff-tree",
    "for-each-ref",
    "grep",
    "help",
    "log",
    "ls-files",
    "ls-remote",
    "ls-tree",
    "merge-base",
    "name-rev",
    "rev-list",
    "rev-parse",
    "shortlog",
    "show",
    "show-ref",
    "status",
    "var",
    "version",
    "whatchanged",
];

/// git subcommands allowed only in their listing forms, checked in [`git`].
pub const GIT_LISTING_SUBCOMMANDS: &[&str] =
    &["branch", "config", "reflog", "remote", "stash", "worktree"];

/// git's global flags that take a value.
const GIT_VALUE_FLAGS: &[&str] = &["-C", "-c", "--git-dir", "--work-tree", "--namespace"];

/// Read-only gh subcommands, matched against the leading positional arguments.
pub const GH_SUBCOMMANDS: &[&[&str]] = &[
    &["api"],
    &["auth", "status"],
    &["issue", "list"],
    &["issue", "status"],
    &["issue", "view"],
    &["pr", "checks"],
    &["pr", "diff"],
    &["pr", "list"],
    &["pr", "status"],
    &["pr", "view"],
    &["release", "list"],
    &["release", "view"],
    &["repo", "view"],
    &["run", "list"],
    &["run", "view"],
    &["search"],
    &["workflow", "list"],
    &["workflow", "view"],
];

/// gh's global flags that take a value.
const GH_VALUE_FLAGS: &[&str] = &["-R", "--repo"];

/// zellij subcommands that read session state or rename panes and tabs.
///
/// Actions that type into another pane (`paste`, `send-keys`, `write-chars`)
/// are left out: a shell in that pane would run whatever they send.
pub const ZELLIJ_SUBCOMMANDS: &[&[&str]] = &[
    &["list-sessions"],
    &["ls"],
    &["action", "dump-screen"],
    &["action", "list-clients"],
    &["action", "list-panes"],
    &["action", "list-tabs"],
    &["action", "query-tab-names"],
    &["action", "rename-pane"],
    &["action", "rename-tab"],
];

/// zellij's global flags that take a value.
const ZELLIJ_VALUE_FLAGS: &[&str] = &[
    "-s",
    "--session",
    "-c",
    "--config",
    "--config-dir",
    "--data-dir",
    "-l",
    "--layout",
];

/// Tools allowed outside any jj repo, whatever their arguments: machine and
/// service tooling that does not produce project files.
pub const OUTSIDE_TOOLS: &[&str] = &[
    "aws", "chezmoi", "claude", "gh", "gpuq", "kubectl", "op-agent", "ssh", "zellij",
];

/// Checks one simple command run from `cwd` outside any jj repo, returning
/// why it is not allowed: what is allowed in a default workspace, plus
/// [`OUTSIDE_TOOLS`] and making or cloning a jj repo.
pub fn check_outside(name: &str, args: &[Arg], cwd: Option<&Path>) -> Result<(), String> {
    if OUTSIDE_TOOLS.contains(&name) {
        return Ok(());
    }
    match name {
        "jj" => {
            let positionals = positionals(args, JJ_VALUE_FLAGS);
            if matches!(positionals.as_slice(), [Some("git"), Some("init" | "clone"), ..]) {
                Ok(())
            } else {
                jj(args, cwd)
            }
        }
        "git" => match positionals(args, GIT_VALUE_FLAGS).as_slice() {
            [Some(subcommand @ ("init" | "clone")), ..] => Err(format!(
                "`git {subcommand}` makes a plain git repo; use `jj git {subcommand}` instead"
            )),
            _ => git(args, cwd),
        },
        _ => check(name, args, cwd),
    }
}

/// Checks one simple command run from `cwd` in the default workspace,
/// returning why it is not allowed.
///
/// `cwd` is `None` when a preceding `cd` made the directory unknowable; it is
/// only used to resolve `jj -R` and `git -C` paths.
pub fn check(name: &str, args: &[Arg], cwd: Option<&Path>) -> Result<(), String> {
    if [READ_ONLY, UNTRACKED_WRITES, SHELL_BUILTINS]
        .iter()
        .any(|list| list.contains(&name))
    {
        return Ok(());
    }
    if let Some((_, flags)) = FORBIDDEN_FLAGS.iter().find(|(command, _)| *command == name) {
        return match args
            .iter()
            .flatten()
            .find(|arg| flags.iter().any(|flag| flag_matches(flag, arg)))
        {
            Some(arg) => Err(format!(
                "`{name} {arg}` can write files or run other commands"
            )),
            None => Ok(()),
        };
    }
    match name {
        "jj" => jj(args, cwd),
        "git" => git(args, cwd),
        "gh" => gh(args),
        "zellij" => zellij(args),
        "sed" => sed(args),
        "uniq" if positionals(args, &[]).len() <= 1 => Ok(()),
        "uniq" => Err("`uniq` with two files writes the second one".to_owned()),
        "command" if has_any(args, &["-v", "-V"]) => Ok(()),
        "command" => Err("`command` without `-v` runs another command".to_owned()),
        "env" if args.is_empty() => Ok(()),
        "env" => Err("`env` with arguments runs another command".to_owned()),
        _ => Err(format!("`{name}` is not on the read-only allowlist")),
    }
}

fn flag_matches(flag: &str, arg: &str) -> bool {
    if flag.starts_with("--") {
        arg == flag
            || arg
                .strip_prefix(flag)
                .is_some_and(|rest| rest.starts_with('='))
    } else if let Some(short) = flag.strip_prefix('-').filter(|short| short.len() == 1) {
        arg.starts_with('-') && !arg.starts_with("--") && arg[1..].contains(short)
    } else {
        arg == flag
    }
}

fn has_any(args: &[Arg], flags: &[&str]) -> bool {
    args.iter()
        .flatten()
        .any(|arg| flags.iter().any(|flag| flag_matches(flag, arg)))
}

/// Exact matching only: a short-flag cluster such as `-mhello` is a value,
/// not a help request.
fn wants_help(args: &[Arg]) -> bool {
    args.iter()
        .flatten()
        .any(|arg| matches!(arg.as_str(), "-h" | "--help" | "--version"))
}

/// The arguments after the first positional one (the subcommand), so its
/// flags are not confused with global flags spelled the same way.
fn after_subcommand<'a>(args: &'a [Arg], value_flags: &[&str]) -> &'a [Arg] {
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        match arg.as_deref() {
            Some(flag) if value_flags.contains(&flag) => index += 2,
            Some(flag) if flag.starts_with('-') && flag != "-" => index += 1,
            _ => return &args[index + 1..],
        }
    }
    &[]
}

/// Positional arguments, skipping flags and the values of `value_flags`.
///
/// Non-literal arguments are kept as `None`: they may be positional.
fn positionals<'a>(args: &'a [Arg], value_flags: &[&str]) -> Vec<Option<&'a str>> {
    let mut positionals = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_deref() {
            Some("--") => {
                positionals.extend(args.map(|arg| arg.as_deref()));
                break;
            }
            Some(flag) if value_flags.contains(&flag) => {
                args.next();
            }
            Some(flag) if flag.starts_with('-') && flag != "-" => {}
            other => positionals.push(other),
        }
    }
    positionals
}

/// The value of the first of `flags` in `args`, in `-f value`, `--flag value`
/// or `--flag=value` form.
fn flag_value<'a>(args: &'a [Arg], flags: &[&str]) -> Option<Option<&'a str>> {
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let Some(arg) = arg.as_deref() else { continue };
        for flag in flags {
            if arg == *flag {
                return Some(args.next().and_then(|value| value.as_deref()));
            }
            if let Some(value) = arg
                .strip_prefix(flag)
                .and_then(|rest| rest.strip_prefix('='))
            {
                return Some(Some(value));
            }
        }
    }
    None
}

/// Whether the leading positionals match one of `allowed`.
///
/// A match needs every word of the allowed prefix to be literal.
fn subcommand_allowed(positionals: &[Option<&str>], allowed: &[&[&str]]) -> bool {
    allowed.iter().any(|prefix| {
        if prefix.is_empty() {
            return positionals.is_empty();
        }
        prefix.len() <= positionals.len()
            && prefix
                .iter()
                .zip(positionals)
                .all(|(want, got)| Some(*want) == *got)
    })
}

/// Whether `dir`, given to `-R` or `-C`, is outside every default workspace,
/// so the command acts on a workspace other than default@.
fn targets_other_workspace(dir: Option<&str>, cwd: Option<&Path>) -> bool {
    let Some(dir) = dir else { return false };
    let dir = Path::new(dir);
    let absolute = match cwd {
        Some(cwd) => workspace::absolutize(cwd, dir),
        None if dir.is_absolute() => workspace::absolutize(Path::new("/"), dir),
        None => return false,
    };
    workspace::default_workspace_root(&absolute).is_none()
}

fn describe(program: &str, positionals: &[Option<&str>]) -> String {
    let words: Vec<&str> = positionals
        .iter()
        .take(2)
        .map(|word| word.unwrap_or("$…"))
        .collect();
    format!("`{program} {}`", words.join(" "))
}

fn jj(args: &[Arg], cwd: Option<&Path>) -> Result<(), String> {
    if wants_help(args) {
        return Ok(());
    }
    if let Some(repo) = flag_value(args, &["-R", "--repository"])
        && targets_other_workspace(repo, cwd)
    {
        return Ok(());
    }
    let mut positionals = positionals(args, JJ_VALUE_FLAGS);
    for (position, alias, command) in JJ_ALIASES {
        if let Some(word) = positionals.get_mut(*position)
            && *word == Some(*alias)
        {
            *word = Some(*command);
        }
    }
    if subcommand_allowed(&positionals, JJ_SUBCOMMANDS) {
        Ok(())
    } else {
        Err(format!(
            "{} is not a read-only jj command",
            describe("jj", &positionals)
        ))
    }
}

fn git(args: &[Arg], cwd: Option<&Path>) -> Result<(), String> {
    if wants_help(args) {
        return Ok(());
    }
    if let Some(dir) = flag_value(args, &["-C"])
        && targets_other_workspace(dir, cwd)
    {
        return Ok(());
    }
    if has_any(args, &["--output"]) {
        return Err("`git --output` writes a file".to_owned());
    }
    let positionals = positionals(args, GIT_VALUE_FLAGS);
    let subcommand_args = after_subcommand(args, GIT_VALUE_FLAGS);
    let listing = |allowed: bool| {
        if allowed {
            Ok(())
        } else {
            Err(format!(
                "{} is not a read-only git command (only its listing form is allowed)",
                describe("git", &positionals)
            ))
        }
    };
    match positionals.as_slice() {
        [] => Ok(()),
        [Some(subcommand), ..] if GIT_SUBCOMMANDS.contains(subcommand) => Ok(()),
        [Some("branch")] => listing(!has_any(
            subcommand_args,
            &[
                "-d",
                "-D",
                "-m",
                "-M",
                "-c",
                "-C",
                "-f",
                "-u",
                "--delete",
                "--move",
                "--copy",
                "--force",
                "--set-upstream-to",
                "--unset-upstream",
                "--edit-description",
                "--track",
                "--no-track",
                "--create-reflog",
            ],
        )),
        [Some("config"), rest @ ..] => {
            let reads = has_any(
                subcommand_args,
                &["--get", "--get-all", "--get-regexp", "--list", "-l"],
            ) || matches!(rest.first(), Some(Some("get" | "list")));
            let writes = has_any(
                subcommand_args,
                &[
                    "--add",
                    "--unset",
                    "--unset-all",
                    "--replace-all",
                    "--rename-section",
                    "--remove-section",
                    "-e",
                    "--edit",
                ],
            );
            listing(reads && !writes)
        }
        [Some("reflog"), rest @ ..] => listing(!matches!(
            rest.first(),
            Some(Some("expire" | "delete") | None)
        )),
        [Some("remote"), rest @ ..] => listing(matches!(
            rest.first(),
            None | Some(Some("show" | "get-url"))
        )),
        [Some("stash"), Some("list" | "show"), ..] | [Some("worktree"), Some("list"), ..] => Ok(()),
        _ => Err(format!(
            "{} is not a read-only git command",
            describe("git", &positionals)
        )),
    }
}

fn gh(args: &[Arg]) -> Result<(), String> {
    if wants_help(args) {
        return Ok(());
    }
    let positionals = positionals(args, GH_VALUE_FLAGS);
    if !subcommand_allowed(&positionals, GH_SUBCOMMANDS) {
        return Err(format!(
            "{} is not a read-only gh command",
            describe("gh", &positionals)
        ));
    }
    if positionals.first() == Some(&Some("api")) {
        // `gh api` switches to POST as soon as it is given fields.
        let method = flag_value(args, &["-X", "--method"]);
        let is_get =
            method.is_none_or(|method| method.is_some_and(|m| m.eq_ignore_ascii_case("GET")));
        let has_body = has_any(args, &["-f", "-F", "--field", "--raw-field", "--input"]);
        if !is_get || has_body {
            return Err("`gh api` is only allowed for GET requests without fields".to_owned());
        }
    }
    Ok(())
}

fn zellij(args: &[Arg]) -> Result<(), String> {
    let positionals = positionals(args, ZELLIJ_VALUE_FLAGS);
    if !subcommand_allowed(&positionals, ZELLIJ_SUBCOMMANDS) {
        return Err(format!(
            "{} is not a read-only zellij command",
            describe("zellij", &positionals)
        ));
    }
    if has_any(args, &["--path"]) {
        return Err("`zellij action dump-screen --path` writes a file".to_owned());
    }
    Ok(())
}

/// Allows `sed` as a stream filter.
///
/// sed scripts can write files (`w`) and run commands (`e`) on their own, so
/// rather than parse the sed language this accepts only the two shapes agents
/// use for reading: a line-range print (`sed -n '1,50p'`) and a single
/// substitution whose flags cannot write or execute (`sed 's/a/b/g'`).
fn sed(args: &[Arg]) -> Result<(), String> {
    const UNSUPPORTED: &str = "`sed` is only allowed as a filter: `sed -n '<from>,<to>p'` or a single `s/…/…/` with flags from `gip0-9IM`";
    let mut scripts = Vec::new();
    let mut first_operand_is_script = true;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let Some(arg) = arg.as_deref() else {
            if first_operand_is_script {
                return Err(UNSUPPORTED.to_owned());
            }
            continue;
        };
        match arg {
            "-e" | "--expression" => {
                first_operand_is_script = false;
                scripts.push(args.next().and_then(|script| script.as_deref()));
            }
            "-n" | "-E" | "-r" | "-s" | "-u" | "-z" | "--quiet" | "--silent" | "--posix"
            | "--regexp-extended" | "--separate" | "--unbuffered" | "--null-data" | "--debug"
            | "--sandbox" => {}
            _ if arg.starts_with("--expression=") => {
                first_operand_is_script = false;
                scripts.push(arg.strip_prefix("--expression="));
            }
            _ if arg.starts_with("-i") || arg.starts_with("--in-place") => {
                return Err("`sed -i` edits files in place".to_owned());
            }
            _ if arg.starts_with('-') && arg.len() > 1 => {
                // Any other cluster, e.g. `-nE`, must be made of the harmless
                // single-letter flags above; `-f` would read an unchecked
                // script from a file.
                if !arg[1..].chars().all(|flag| "nErsuz".contains(flag)) {
                    return Err(UNSUPPORTED.to_owned());
                }
            }
            script if first_operand_is_script => {
                first_operand_is_script = false;
                scripts.push(Some(script));
            }
            _ => {}
        }
    }
    if !scripts.is_empty()
        && scripts
            .iter()
            .all(|script| script.is_some_and(is_filter_script))
    {
        Ok(())
    } else {
        Err(UNSUPPORTED.to_owned())
    }
}

fn is_filter_script(script: &str) -> bool {
    let script = script.trim();
    is_line_print(script) || is_plain_substitution(script)
}

/// `N`, `N,M` or `$`-anchored line ranges followed by `p` or `q`.
fn is_line_print(script: &str) -> bool {
    let Some(range) = script.strip_suffix(['p', 'q']) else {
        return false;
    };
    let is_line = |line: &str| {
        let line = line.trim();
        line == "$" || (!line.is_empty() && line.chars().all(|c| c.is_ascii_digit()))
    };
    match range.split_once(',') {
        Some((from, to)) => is_line(from) && is_line(to),
        None => is_line(range),
    }
}

/// `s<d>pattern<d>replacement<d>flags` with flags that neither write (`w`)
/// nor execute (`e`).
fn is_plain_substitution(script: &str) -> bool {
    let mut chars = script.chars();
    if chars.next() != Some('s') {
        return false;
    }
    let Some(delimiter) = chars
        .next()
        .filter(|d| !d.is_alphanumeric() && *d != '\\' && *d != '\n')
    else {
        return false;
    };
    let mut delimiters_seen = 1;
    let mut escaped = false;
    for c in chars.by_ref() {
        match c {
            _ if escaped => escaped = false,
            '\\' => escaped = true,
            '\n' => return false,
            _ if c == delimiter => {
                delimiters_seen += 1;
                if delimiters_seen == 3 {
                    break;
                }
            }
            _ => {}
        }
    }
    delimiters_seen == 3 && chars.all(|flag| "gipIM0123456789".contains(flag))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::{check, is_filter_script};

    fn args(line: &str) -> Vec<super::Arg> {
        line.split_whitespace()
            .map(|arg| Some(arg.to_owned()))
            .collect()
    }

    #[rstest]
    #[case("1,50p")]
    #[case("$p")]
    #[case("10q")]
    #[case("s/^/  /")]
    #[case("s|a/b|c|g")]
    #[case(r"s/a\/b/c/2")]
    fn filter_scripts(#[case] script: &str) {
        assert!(is_filter_script(script), "{script}");
    }

    #[rstest]
    #[case("s/a/b/w out.txt")]
    #[case("s/a/b/e")]
    #[case("1d")]
    #[case("w out.txt")]
    #[case("s/a/b")]
    #[case("1,5p;w x")]
    fn non_filter_scripts(#[case] script: &str) {
        assert!(!is_filter_script(script), "{script}");
    }

    #[rstest]
    #[case("sort", "-u names.txt")]
    #[case("sort", "-k2,2n names.txt")]
    #[case("fd", "-e rs")]
    #[case("find", ". -name *.rs -type f")]
    #[case("jj", "")]
    #[case("jj", "--no-pager log -r trunk()..@")]
    #[case("jj", "--color never op log")]
    #[case("jj", "b l")]
    #[case("jj", "bookmark l")]
    #[case("jj", "st")]
    #[case("jj", "new --help")]
    #[case("git", "log --oneline -5")]
    #[case("git", "-c color.ui=never diff HEAD~1")]
    #[case("git", "branch -a")]
    #[case("git", "-c color.ui=never branch -a")]
    #[case("git", "config --get user.name")]
    #[case("git", "config get user.name")]
    #[case("git", "remote -v")]
    #[case("git", "reflog")]
    #[case("gh", "pr view 12 --json title")]
    #[case("gh", "-R o/r issue list")]
    #[case("gh", "api repos/o/r/pulls --paginate")]
    #[case("gh", "api -X GET search/issues")]
    #[case("zellij", "action rename-pane --pane-id terminal_3 sorter")]
    #[case("zellij", "--session main action list-panes -a")]
    #[case("sed", "-n 1,50p src/main.rs")]
    #[case("sed", "-nE -e s/a/b/ file")]
    #[case("uniq", "-c")]
    #[case("command", "-v jj")]
    fn allowed(#[case] name: &str, #[case] line: &str) {
        assert_eq!(check(name, &args(line), None), Ok(()), "{name} {line}");
    }

    #[rstest]
    #[case("python3", "script.py")]
    #[case("sort", "-o out.txt in.txt")]
    #[case("sort", "--output=out.txt in.txt")]
    #[case("find", ". -name *.orig -delete")]
    #[case("find", ". -exec rm {} ;")]
    #[case("fd", "-e orig -x rm")]
    #[case("rg", "--pre cat foo")]
    #[case("jj", "new")]
    #[case("jj", "describe -m wip")]
    #[case("jj", "describe -mhello")]
    #[case("git", "-c color.ui=never branch -D old")]
    #[case("jj", "--no-pager squash")]
    #[case("jj", "workspace forget other")]
    #[case("jj", "git fetch")]
    #[case("git", "commit -m x")]
    #[case("git", "checkout main")]
    #[case("git", "branch -D old")]
    #[case("git", "branch new-branch")]
    #[case("git", "config user.name x")]
    #[case("git", "reflog expire --all")]
    #[case("git", "diff --output=patch.diff")]
    #[case("gh", "pr create")]
    #[case("gh", "api -X POST repos/o/r/issues")]
    #[case("gh", "api repos/o/r/issues -f title=x")]
    #[case("zellij", "action paste -p 3 rm")]
    #[case("zellij", "action dump-screen --path screen.txt")]
    #[case("zellij", "run -- cargo build")]
    #[case("sed", "-i s/a/b/ f.txt")]
    #[case("sed", "s/a/b/w out.txt f.txt")]
    #[case("sed", "-f script.sed f.txt")]
    #[case("uniq", "in.txt out.txt")]
    #[case("command", "rm x")]
    #[case("env", "rm x")]
    fn denied(#[case] name: &str, #[case] line: &str) {
        assert!(check(name, &args(line), None).is_err(), "{name} {line}");
    }

    #[test]
    fn non_literal_subcommand_is_denied() {
        let args = vec![None, Some("-m".to_owned()), Some("wip".to_owned())];
        assert!(check("jj", &args, None).is_err());
    }
}
