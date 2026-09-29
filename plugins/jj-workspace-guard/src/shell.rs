//! Deciding whether a Bash command stays read-only in the default workspace.
//!
//! The command is parsed into a bash syntax tree and every simple command in
//! it, including those inside substitutions, loops and subshells, is checked
//! against [`crate::commands`]. Output redirects are checked by where they
//! write. `cd` is followed, so `cd <workspace> && make` is judged where `make`
//! actually runs.

use std::path::{Path, PathBuf};

use brush_parser::ParserOptions;
use brush_parser::ast::{
    AndOrList, Command, CommandPrefixOrSuffixItem, CompoundCommand, CompoundList, CompoundListItem,
    ExtendedTestExpr, IoFileRedirectKind, IoFileRedirectTarget, IoRedirect, Pipeline,
    PipelineOperator, SeparatorOperator, SimpleCommand, Word,
};
use brush_parser::word::{TildeExpr, WordPiece, WordPieceWithSource};

use crate::commands::{self, Arg};
use crate::message::Denial;
use crate::workspace;

/// Checks `command` run from `cwd`, returning a denial if any part of it could
/// write inside a default jj workspace.
pub fn check(command: &str, cwd: &Path) -> Result<(), Denial> {
    let mut checker = Checker {
        options: ParserOptions::default(),
        cwd: Some(cwd.to_path_buf()),
        starting_root: workspace::default_workspace_root(cwd),
    };
    checker.program(command, "the command")
}

struct Checker {
    options: ParserOptions,
    /// Where the next command runs, or `None` once a `cd` made that unknowable.
    cwd: Option<PathBuf>,
    /// The default workspace the command started in, if any. When `cwd` is
    /// unknown the command is judged as if it were still there.
    starting_root: Option<PathBuf>,
}

impl Checker {
    /// The default workspace the next command runs in, if any.
    fn default_root(&self) -> Option<PathBuf> {
        match &self.cwd {
            Some(cwd) => workspace::default_workspace_root(cwd),
            None => self.starting_root.clone(),
        }
    }

    /// Blocks the construct being checked if it runs in a default workspace.
    fn refuse(&self, reason: impl Into<String>) -> Result<(), Denial> {
        match self.default_root() {
            Some(default_root) => Err(Denial {
                reason: reason.into(),
                default_root,
            }),
            None => Ok(()),
        }
    }

    // Directory tracking. A `cd` lasts until the end of the shell it runs in,
    // but not every construct runs in the same shell, or runs at all.

    /// Checks code that runs in a subshell, where a `cd` does not outlast it.
    fn in_subshell(
        &mut self,
        check: impl FnOnce(&mut Self) -> Result<(), Denial>,
    ) -> Result<(), Denial> {
        let saved = self.cwd.clone();
        let result = check(self);
        self.cwd = saved;
        result
    }

    /// Checks code that may not run, such as a loop body or an `if` branch.
    /// A `cd` inside leaves the directory unknown afterwards.
    fn maybe_run(
        &mut self,
        check: impl FnOnce(&mut Self) -> Result<(), Denial>,
    ) -> Result<(), Denial> {
        let saved = self.cwd.clone();
        let result = check(self);
        if self.cwd != saved {
            self.cwd = None;
        }
        result
    }

    fn cd(&mut self, args: &[Arg]) {
        let operands: Vec<&Arg> = args
            .iter()
            .filter(|arg| !matches!(arg, Some(flag) if flag.starts_with('-') && flag != "-"))
            .collect();
        self.cwd = match operands.as_slice() {
            [] => home_dir(),
            [Some(target)] if target != "-" => {
                let target = Path::new(target);
                match &self.cwd {
                    Some(cwd) => Some(workspace::absolutize(cwd, target)),
                    None if target.is_absolute() => {
                        Some(workspace::absolutize(Path::new("/"), target))
                    }
                    None => None,
                }
            }
            _ => None,
        };
    }

    // The syntax tree, top down.

    fn program(&mut self, source: &str, what: &str) -> Result<(), Denial> {
        let parsed = brush_parser::Parser::new(source.as_bytes(), &self.options).parse_program();
        match parsed {
            Ok(program) => program
                .complete_commands
                .iter()
                .try_for_each(|list| self.list(list)),
            Err(_) => self.refuse(format!(
                "{what} could not be parsed as bash, so the guard cannot tell whether it is \
                 read-only"
            )),
        }
    }

    fn list(&mut self, list: &CompoundList) -> Result<(), Denial> {
        for CompoundListItem(and_or, separator) in &list.0 {
            match separator {
                SeparatorOperator::Async => self.in_subshell(|checker| checker.and_or(and_or))?,
                SeparatorOperator::Sequence => self.and_or(and_or)?,
            }
        }
        Ok(())
    }

    fn and_or(&mut self, list: &AndOrList) -> Result<(), Denial> {
        let pipelines: Vec<(PipelineOperator, &Pipeline)> = list.into_iter().collect();
        for (index, (operator, pipeline)) in pipelines.iter().enumerate() {
            // Next to `||`, a pipeline's `cd` may or may not have happened:
            // in `cd dir || cmd`, cmd runs only if the `cd` failed.
            let beside_or = matches!(operator, PipelineOperator::Or)
                || matches!(pipelines.get(index + 1), Some((PipelineOperator::Or, _)));
            if beside_or {
                self.maybe_run(|checker| checker.pipeline(pipeline))?;
            } else {
                self.pipeline(pipeline)?;
            }
        }
        Ok(())
    }

    fn pipeline(&mut self, pipeline: &Pipeline) -> Result<(), Denial> {
        match pipeline.seq.as_slice() {
            [command] => self.command(command),
            // Every stage of a multi-command pipeline runs in its own subshell.
            stages => stages
                .iter()
                .try_for_each(|command| self.in_subshell(|checker| checker.command(command))),
        }
    }

    fn command(&mut self, command: &Command) -> Result<(), Denial> {
        match command {
            Command::Simple(simple) => self.simple(simple),
            Command::Compound(compound, redirects) => {
                self.compound(compound)?;
                redirects
                    .iter()
                    .flat_map(|list| &list.0)
                    .try_for_each(|redirect| self.redirect(redirect))
            }
            Command::Function(_) => self.refuse("defining a shell function is not allowed"),
            Command::ExtendedTest(test, redirects) => {
                self.test_expr(&test.expr)?;
                redirects
                    .iter()
                    .flat_map(|list| &list.0)
                    .try_for_each(|redirect| self.redirect(redirect))
            }
        }
    }

    fn compound(&mut self, compound: &CompoundCommand) -> Result<(), Denial> {
        match compound {
            CompoundCommand::Arithmetic(arithmetic) => self.opaque(&arithmetic.expr.value),
            CompoundCommand::ArithmeticForClause(clause) => {
                [&clause.initializer, &clause.condition, &clause.updater]
                    .into_iter()
                    .flatten()
                    .try_for_each(|expr| self.opaque(&expr.value))?;
                self.maybe_run(|checker| checker.list(&clause.body.list))
            }
            CompoundCommand::BraceGroup(group) => self.list(&group.list),
            CompoundCommand::Subshell(subshell) => {
                self.in_subshell(|checker| checker.list(&subshell.list))
            }
            CompoundCommand::ForClause(clause) => {
                for value in clause.values.iter().flatten() {
                    self.word(value)?;
                }
                self.maybe_run(|checker| checker.list(&clause.body.list))
            }
            CompoundCommand::CaseClause(clause) => {
                self.word(&clause.value)?;
                for case in &clause.cases {
                    for pattern in &case.patterns {
                        self.word(pattern)?;
                    }
                    if let Some(body) = &case.cmd {
                        self.maybe_run(|checker| checker.list(body))?;
                    }
                }
                Ok(())
            }
            CompoundCommand::IfClause(clause) => self.maybe_run(|checker| {
                checker.list(&clause.condition)?;
                checker.list(&clause.then)?;
                for branch in clause.elses.iter().flatten() {
                    if let Some(condition) = &branch.condition {
                        checker.list(condition)?;
                    }
                    checker.list(&branch.body)?;
                }
                Ok(())
            }),
            CompoundCommand::WhileClause(clause) | CompoundCommand::UntilClause(clause) => self
                .maybe_run(|checker| {
                    checker.list(&clause.0)?;
                    checker.list(&clause.1.list)
                }),
            CompoundCommand::Coprocess(_) => self.refuse("starting a coprocess is not allowed"),
        }
    }

    fn test_expr(&mut self, expr: &ExtendedTestExpr) -> Result<(), Denial> {
        match expr {
            ExtendedTestExpr::And(left, right) | ExtendedTestExpr::Or(left, right) => {
                self.test_expr(left)?;
                self.test_expr(right)
            }
            ExtendedTestExpr::Not(inner) | ExtendedTestExpr::Parenthesized(inner) => {
                self.test_expr(inner)
            }
            ExtendedTestExpr::UnaryTest(_, operand) => self.word(operand).map(drop),
            ExtendedTestExpr::BinaryTest(_, left, right) => {
                self.word(left)?;
                self.word(right).map(drop)
            }
        }
    }

    fn simple(&mut self, command: &SimpleCommand) -> Result<(), Denial> {
        for item in command.prefix.iter().flat_map(|prefix| &prefix.0) {
            self.item(item)?;
        }
        let name = match &command.word_or_name {
            Some(name) => self.word(name)?,
            // Only assignments and redirects, already checked.
            None => return Ok(()),
        };
        let mut args = Vec::new();
        for item in command.suffix.iter().flat_map(|suffix| &suffix.0) {
            if let Some(arg) = self.item(item)? {
                args.push(arg);
            }
        }

        if self.default_root().is_some() {
            let Some(name) = name.as_deref() else {
                return self.refuse(format!(
                    "the command name `{}` is not a literal word",
                    command
                        .word_or_name
                        .as_ref()
                        .map_or("", |word| word.value.as_str())
                ));
            };
            if let Err(reason) = commands::check(name, &args, self.cwd.as_deref()) {
                return self.refuse(reason);
            }
        }
        // Follow `cd` even outside default@: it may lead into it.
        if name.as_deref() == Some("cd") {
            self.cd(&args);
        }
        Ok(())
    }

    /// Checks a prefix or suffix item, returning it as an argument if it is one.
    fn item(&mut self, item: &CommandPrefixOrSuffixItem) -> Result<Option<Arg>, Denial> {
        match item {
            CommandPrefixOrSuffixItem::IoRedirect(redirect) => {
                self.redirect(redirect)?;
                Ok(None)
            }
            CommandPrefixOrSuffixItem::Word(word)
            | CommandPrefixOrSuffixItem::AssignmentWord(_, word) => self.word(word).map(Some),
            CommandPrefixOrSuffixItem::ProcessSubstitution(_, subshell) => {
                self.in_subshell(|checker| checker.list(&subshell.list))?;
                Ok(Some(None))
            }
        }
    }

    fn redirect(&mut self, redirect: &IoRedirect) -> Result<(), Denial> {
        match redirect {
            IoRedirect::File(_, kind, target) => match target {
                IoFileRedirectTarget::Fd(_) => Ok(()),
                IoFileRedirectTarget::ProcessSubstitution(_, subshell) => {
                    self.in_subshell(|checker| checker.list(&subshell.list))
                }
                IoFileRedirectTarget::Filename(word) => {
                    let target = self.word(word)?;
                    match kind {
                        IoFileRedirectKind::Read | IoFileRedirectKind::DuplicateInput => Ok(()),
                        IoFileRedirectKind::Write
                        | IoFileRedirectKind::Append
                        | IoFileRedirectKind::ReadAndWrite
                        | IoFileRedirectKind::Clobber
                        | IoFileRedirectKind::DuplicateOutput => {
                            self.write_target(target.as_deref(), &word.value)
                        }
                    }
                }
                // `>&2` duplicates a descriptor, but `>&file` writes to a file.
                IoFileRedirectTarget::Duplicate(word) => match self.word(word)? {
                    Some(fd) if fd == "-" || fd.chars().all(|c| c.is_ascii_digit()) => Ok(()),
                    target => self.write_target(target.as_deref(), &word.value),
                },
            },
            IoRedirect::HereDocument(_, heredoc) => {
                if heredoc.requires_expansion {
                    match brush_parser::word::parse_heredoc(&heredoc.doc.value, &self.options) {
                        Ok(pieces) => {
                            self.pieces(&heredoc.doc.value, &pieces)?;
                        }
                        Err(_) => self.refuse("a here-document could not be parsed")?,
                    }
                }
                Ok(())
            }
            IoRedirect::HereString(_, word) => self.word(word).map(drop),
            IoRedirect::OutputAndError(word, _) => {
                let target = self.word(word)?;
                self.write_target(target.as_deref(), &word.value)
            }
        }
    }

    /// Blocks an output redirect into a default workspace, wherever the
    /// command itself runs.
    fn write_target(&self, target: Option<&str>, source: &str) -> Result<(), Denial> {
        let Some(target) = target else {
            return self.refuse(format!(
                "the redirect target `{source}` is not a literal path, so the guard cannot tell \
                 where it writes"
            ));
        };
        let path = match &self.cwd {
            Some(cwd) => workspace::absolutize(cwd, Path::new(target)),
            None if Path::new(target).is_absolute() => {
                workspace::absolutize(Path::new("/"), Path::new(target))
            }
            None => {
                return self.refuse(format!(
                    "the redirect to `{target}` is relative to a directory the guard cannot \
                     determine"
                ));
            }
        };
        match workspace::default_workspace_written(&path) {
            Some(default_root) => Err(Denial {
                reason: format!("the redirect to `{target}` writes {}", path.display()),
                default_root,
            }),
            None => Ok(()),
        }
    }

    // Words. A word can hide commands in `$(…)`, backticks or `<(…)`; those
    // are checked like any other command. Its literal value, when it has one,
    // is what the command sees.

    fn word(&mut self, word: &Word) -> Result<Arg, Denial> {
        match brush_parser::word::parse(&word.value, &self.options) {
            Ok(pieces) => self.pieces(&word.value, &pieces),
            Err(_) => {
                self.refuse(format!(
                    "`{}` could not be parsed as a bash word",
                    word.value
                ))?;
                Ok(None)
            }
        }
    }

    fn pieces(&mut self, source: &str, pieces: &[WordPieceWithSource]) -> Result<Arg, Denial> {
        let mut literal = Some(String::new());
        for piece in pieces {
            let text = match &piece.piece {
                WordPiece::Text(text)
                | WordPiece::SingleQuotedText(text)
                | WordPiece::AnsiCQuotedText(text) => Some(text.clone()),
                WordPiece::EscapeSequence(escaped) => {
                    Some(escaped.strip_prefix('\\').unwrap_or(escaped).to_owned())
                }
                WordPiece::DoubleQuotedSequence(inner)
                | WordPiece::GettextDoubleQuotedSequence(inner) => self.pieces(source, inner)?,
                WordPiece::TildeExpansion(TildeExpr::Home) => {
                    home_dir().map(|home| home.to_string_lossy().into_owned())
                }
                WordPiece::TildeExpansion(_) => None,
                WordPiece::CommandSubstitution(inner)
                | WordPiece::BackquotedCommandSubstitution(inner) => {
                    self.in_subshell(|checker| {
                        checker.program(inner, &format!("the command substitution `{inner}`"))
                    })?;
                    None
                }
                WordPiece::ParameterExpansion(_) | WordPiece::ArithmeticExpression(_) => {
                    let raw = source
                        .get(piece.start_index..piece.end_index)
                        .unwrap_or(source);
                    self.opaque(raw)?;
                    None
                }
            };
            literal = literal.zip(text).map(|(mut literal, text)| {
                literal.push_str(&text);
                literal
            });
        }
        Ok(literal)
    }

    /// Blocks a command nested inside `${…}` or `$((…))`, where the parser
    /// does not break it out for checking.
    fn opaque(&self, raw: &str) -> Result<(), Denial> {
        if ["$(", "`", "<(", ">("]
            .iter()
            .any(|opener| raw.contains(opener))
        {
            self.refuse(format!(
                "`{raw}` nests a command inside an expansion, which the guard cannot check"
            ))
        } else {
            Ok(())
        }
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::check;
    use crate::workspace::tests::{Layout, layout};

    fn expand(command: &str, layout: &Layout) -> String {
        command
            .replace("{root}", &layout.root.to_string_lossy())
            .replace("{added}", &layout.added.to_string_lossy())
            .replace("{outside}", &layout.outside.to_string_lossy())
    }

    #[rstest]
    #[case("ls -la")]
    #[case("rg -n 'foo|bar' src")]
    #[case("jq '.[] | .name' data.json")]
    #[case("cat Cargo.toml | head -5")]
    #[case("git log --oneline -5 && git status --short")]
    #[case("jj log -r 'trunk()..@' --no-graph")]
    #[case("jj workspace add workspaces/new -r 'trunk()'")]
    #[case("ls > /dev/null 2>&1")]
    #[case("ls &>/dev/null")]
    #[case("ls 2>&1 >&2")]
    #[case("ls > {outside}/listing.txt")]
    #[case("ls > {added}/listing.txt")]
    #[case("for f in *.json; do jq . \"$f\"; done")]
    #[case("sed -n '1,50p' src/main.rs")]
    #[case("grep -r foo src | sed 's/^/  /' | sort | uniq -c")]
    #[case("echo \"$(git rev-parse HEAD)\"")]
    #[case("wc -l $(fd -e rs)")]
    #[case("diff <(jj file show -r @- a.rs) a.rs")]
    #[case("set -euo pipefail; ls")]
    #[case("[ -f x ] && cat x || echo missing")]
    #[case("[[ -n \"$HOME\" ]] && echo \"${HOME:-none}\"")]
    #[case("cat <<'EOF'\nhello > not-a-redirect\nEOF")]
    #[case("cd {added} && sed -i s/a/b/ f.txt")]
    #[case("cd workspaces/mine && cargo build")]
    #[case("(cd {added} && rm -rf target)")]
    #[case("jj -R {added} new")]
    #[case("git -C {outside} commit -m x")]
    #[case("FOO=1 printenv FOO")]
    #[case("mkdir -p workspaces && jj workspace add workspaces/new")]
    #[case("printf '/workspaces/\\n' >> .git/info/exclude")]
    fn allowed_in_default(layout: Layout, #[case] command: &str) {
        let command = expand(command, &layout);
        assert_eq!(check(&command, &layout.root), Ok(()), "{command}");
    }

    #[rstest]
    #[case("sed -i s/a/b/ f.txt")]
    #[case("echo hi > notes.txt")]
    #[case("echo hi >> {root}/notes.txt")]
    #[case("echo '[core]' >> .git/config")]
    #[case("ls >& listing.txt")]
    #[case("ls &> listing.txt")]
    #[case("python3 script.py")]
    #[case("jj new")]
    #[case("jj describe -m wip")]
    #[case("git commit -m x")]
    #[case("cargo build")]
    #[case("rm -rf target")]
    #[case("echo $(rm notes.txt)")]
    #[case("echo `rm notes.txt`")]
    #[case("cat <(rm notes.txt)")]
    #[case("cat <<EOF\n$(rm notes.txt)\nEOF")]
    #[case("echo ${x:-$(rm notes.txt)}")]
    #[case("echo $(( $(rm notes.txt) + 1 ))")]
    #[case("ls | xargs rm")]
    #[case("cat in.txt | tee out.txt")]
    #[case("cat > notes.txt <<EOF\nx\nEOF")]
    #[case("ls > \"$OUT\"")]
    #[case("$EDITOR notes.txt")]
    #[case("eval ls")]
    #[case("f() { ls; }; f")]
    #[case("cd {added} || sed -i s/a/b/ f.txt")]
    #[case("(cd {added}); sed -i s/a/b/ f.txt")]
    #[case("cd {added} | true; sed -i s/a/b/ f.txt")]
    #[case("cd {added} && cd ../.. && sed -i s/a/b/ f.txt")]
    #[case("if false; then cd {added}; fi; rm notes.txt")]
    #[case("cd {added} & rm notes.txt")]
    #[case("jj -R {root} new")]
    #[case("if [ -f x ]; then")]
    fn denied_in_default(layout: Layout, #[case] command: &str) {
        let command = expand(command, &layout);
        let denial = check(&command, &layout.root).expect_err(&command);
        assert_eq!(denial.default_root, layout.root, "{command}");
    }

    #[rstest]
    #[case("rm -rf target && cargo build")]
    #[case("sed -i s/a/b/ f.txt")]
    #[case("echo hi > notes.txt")]
    #[case("if [ -f x ]; then")]
    fn anything_goes_elsewhere(layout: Layout, #[case] command: &str) {
        assert_eq!(check(command, &layout.added), Ok(()), "{command}");
        assert_eq!(check(command, &layout.outside), Ok(()), "{command}");
    }

    #[rstest]
    #[case("echo hi > {root}/notes.txt")]
    #[case("cd {root} && jj new")]
    #[case("cd ../.. && rm notes.txt")]
    fn reaching_into_default_from_elsewhere_is_denied(layout: Layout, #[case] command: &str) {
        let command = expand(command, &layout);
        let denial = check(&command, &layout.added).expect_err(&command);
        assert_eq!(denial.default_root, layout.root, "{command}");
    }
}
