# jj-workspace-guard allowlist

Generated from `src/commands.rs`; `UPDATE_ALLOWLIST=1 cargo nextest run` rewrites it, and a test fails while it is out of date.

Read, Grep, Glob and other tools that do not edit files are never affected. Bash commands are checked piece by piece, so pipes, loops and command substitution are allowed when every command in them is.

## In default@ (the default jj workspace)

- b2sum base64 basename cat cksum cmp column comm cut date df diff dirname du echo egrep false fgrep fold free grep head hexdump hostname id jq join ls md5sum nl nproc od paste pgrep printenv printf ps pwd readlink realpath rev seq sha1sum sha256sum sha512sum stat strings tac tail test [ tr true type uname uptime wc which whoami mkdir
- shell builtins: : cd declare exit export local read set shift unset wait
- fd (without -x -X --exec --exec-batch), file (without -C --compile), find (without -delete -exec -execdir -fls -fprint -fprint0 -fprintf -ok -okdir), rg (without --pre), sort (without -o --output), tree (without -o)
- sed as a filter only (`sed -n '1,50p' file`, `sed 's/a/b/g'`), uniq with at most one file, command -v, env with no arguments
- cp mv rm rmdir touch unlink on files in Claude's memory, or in a temporary folder with no repo in it (cp may copy from anywhere)
- rm, rmdir, and mv as a source, on the root folder of a workspace its repo no longer lists, after `jj workspace forget`
- jj log, status, diff, show, evolog, interdiff, root, help, version, op log, op show, op diff, file list, file show, file annotate, file search, bookmark list, tag list, config list, config get, config path, git remote list, git root, workspace list, workspace root, workspace add
- git annotate, blame, cat-file, check-attr, check-ignore, count-objects, describe, diff, diff-files, diff-index, diff-tree, for-each-ref, grep, help, log, ls-files, ls-remote, ls-tree, merge-base, name-rev, rev-list, rev-parse, shortlog, show, show-ref, status, var, version, whatchanged, and the listing forms of branch, config, reflog, remote, stash, worktree
- gh api, auth status, issue list, issue status, issue view, pr checks, pr diff, pr list, pr status, pr view, release list, release view, repo view, run list, run view, search, workflow list, workflow view (api: GET only)
- zellij list-sessions, ls, action dump-screen, action list-clients, action list-panes, action list-tabs, action query-tab-names, action rename-pane, action rename-tab
- output redirects that write outside default@, such as /dev/null or a scratch directory
- Edit and Write only on the repo's git exclude file

## Outside any jj repo

- b2sum base64 basename cat cksum cmp column comm cut date df diff dirname du echo egrep false fgrep fold free grep head hexdump hostname id jq join ls md5sum nl nproc od paste pgrep printenv printf ps pwd readlink realpath rev seq sha1sum sha256sum sha512sum stat strings tac tail test [ tr true type uname uptime wc which whoami mkdir
- shell builtins: : cd declare exit export local read set shift unset wait
- fd (without -x -X --exec --exec-batch), file (without -C --compile), find (without -delete -exec -execdir -fls -fprint -fprint0 -fprintf -ok -okdir), rg (without --pre), sort (without -o --output), tree (without -o)
- sed as a filter only (`sed -n '1,50p' file`, `sed 's/a/b/g'`), uniq with at most one file, command -v, env with no arguments
- cp mv rm rmdir touch unlink on files in Claude's memory, or in a temporary folder with no repo in it (cp may copy from anywhere)
- rm, rmdir, and mv as a source, on the root folder of a workspace its repo no longer lists, after `jj workspace forget`
- jj git init, jj git clone, and jj log, status, diff, show, evolog, interdiff, root, help, version, op log, op show, op diff, file list, file show, file annotate, file search, bookmark list, tag list, config list, config get, config path, git remote list, git root, workspace list, workspace root, workspace add
- git annotate, blame, cat-file, check-attr, check-ignore, count-objects, describe, diff, diff-files, diff-index, diff-tree, for-each-ref, grep, help, log, ls-files, ls-remote, ls-tree, merge-base, name-rev, rev-list, rev-parse, shortlog, show, show-ref, status, var, version, whatchanged (but not git init or git clone: use jj git init and jj git clone)
- machine and service tools: aws, chezmoi, claude, gh, gpuq, kubectl, op-agent, ssh, zellij
- output redirects that write only to temporary folders (such as /tmp or /var/tmp) or Claude's memory
- Edit and Write only in temporary folders and Claude's memory
