//! Finding which jj workspace a path belongs to.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock, PoisonError};

/// Returns the root of the default jj workspace containing `path`, or `None`
/// when `path` is in an added workspace or outside any jj repo.
///
/// jj keeps the repo store, `.jj/repo/`, in the workspace the repo was created
/// in, which is `default@`. Every workspace added later holds a `.jj/repo`
/// *file* pointing back at that store instead. So the nearest `.jj` above
/// `path` identifies the workspace that owns it, and whether that is the
/// default one.
///
/// `path` must be absolute but need not exist, since a write may create it.
/// Symlinks in its longest existing prefix are resolved.
///
/// A repo whose jj repo config sets `jj-workspace-guard.enabled = false` is
/// never guarded: that is for repos that are not projects, such as a home
/// directory or a dotfiles source tracked with jj.
pub fn default_workspace_root(path: &Path) -> Option<PathBuf> {
    let resolved = resolve_existing_prefix(path);
    let workspace_root = resolved.ancestors().find(|dir| dir.join(".jj").is_dir())?;
    (workspace_root.join(".jj/repo").is_dir() && guarded(workspace_root))
        .then(|| workspace_root.to_path_buf())
}

/// Whether the repo at `root` has not opted out of the guard. Asks jj, so
/// the setting is read wherever jj keeps repo config; a failure to ask counts
/// as guarded. Cached because one hook call may check several paths.
fn guarded(root: &Path) -> bool {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, bool>>> = OnceLock::new();
    let mut cache = CACHE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    *cache.entry(root.to_path_buf()).or_insert_with(|| {
        Command::new("jj")
            .args(["--ignore-working-copy", "-R"])
            .arg(root)
            .args(["config", "get", "jj-workspace-guard.enabled"])
            .output()
            .map_or(true, |output| String::from_utf8_lossy(&output.stdout).trim() != "false")
    })
}

/// Returns the root of the default jj workspace that writing `path` would
/// change, or `None` if the write is allowed.
///
/// That is [`default_workspace_root`], with two exceptions. The repo's git
/// exclude file is local, untracked configuration, and an agent in default@
/// needs to exclude the `workspaces/` directory it is about to create.
/// Claude Code's per-project memory directory is where Claude Code tells
/// agents to write, wherever they work.
pub fn default_workspace_written(path: &Path) -> Option<PathBuf> {
    if std::env::var_os("HOME").is_some_and(|home| is_claude_memory(path, Path::new(&home))) {
        return None;
    }
    let root = default_workspace_root(path)?;
    let resolved = resolve_existing_prefix(path);
    let exclude_files = [".git/info/exclude", ".jj/repo/store/git/info/exclude"];
    (!exclude_files.iter().any(|file| resolved == root.join(file))).then_some(root)
}

/// Whether `path` is inside `<home>/.claude/projects/<project>/memory/`.
fn is_claude_memory(path: &Path, home: &Path) -> bool {
    resolve_existing_prefix(path)
        .strip_prefix(resolve_existing_prefix(&home.join(".claude/projects")))
        .is_ok_and(|rest| rest.components().nth(1).is_some_and(|dir| dir.as_os_str() == "memory"))
}

/// Joins `path` onto `base` and removes `.` and `..` components lexically.
pub fn absolutize(base: &Path, path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in base.join(path).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other),
        }
    }
    normalized
}

fn resolve_existing_prefix(path: &Path) -> PathBuf {
    for existing in path.ancestors() {
        if let Ok(canonical) = existing.canonicalize() {
            return match path.strip_prefix(existing) {
                Ok(rest) if !rest.as_os_str().is_empty() => canonical.join(rest),
                _ => canonical,
            };
        }
    }
    path.to_path_buf()
}

#[cfg(test)]
pub(crate) mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use rstest::{fixture, rstest};
    use tempfile::TempDir;

    use super::{absolutize, default_workspace_root, default_workspace_written, is_claude_memory};

    #[test]
    fn claude_memory_is_recognised_and_nothing_else_under_claude() {
        let home = tempfile::tempdir().unwrap();
        let home = home.path();
        let projects = home.join(".claude/projects");
        assert!(is_claude_memory(&projects.join("-home-dev-repo/memory/note.md"), home));
        assert!(is_claude_memory(&projects.join("-home-dev-repo/memory/MEMORY.md"), home));
        assert!(!is_claude_memory(&projects.join("-home-dev-repo/transcript.jsonl"), home));
        assert!(!is_claude_memory(&home.join(".claude/settings.json"), home));
        assert!(!is_claude_memory(&home.join("repo/memory/note.md"), home));
    }

    #[test]
    fn a_repo_can_opt_out_through_its_jj_config() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let jj = |root: &std::path::Path, args: &[&str]| {
            std::fs::create_dir_all(root).unwrap();
            let output = std::process::Command::new("jj")
                .current_dir(root)
                .args(args)
                .output()
                .unwrap();
            assert!(output.status.success(), "jj {args:?}");
        };
        let guarded = base.join("project");
        jj(&guarded, &["git", "init", "."]);
        let opted_out = base.join("dotfiles");
        jj(&opted_out, &["git", "init", "."]);
        jj(&opted_out, &["config", "set", "--repo", "jj-workspace-guard.enabled", "false"]);

        assert_eq!(default_workspace_root(&guarded.join("src/main.rs")), Some(guarded));
        assert_eq!(default_workspace_root(&opted_out.join("nu/config.nu")), None);
    }

    /// A default workspace at `root`, an added workspace nested at
    /// `root/workspaces/mine` (the layout the block message recommends), and a
    /// directory outside any repo.
    pub(crate) struct Layout {
        _dir: TempDir,
        pub root: PathBuf,
        pub added: PathBuf,
        pub outside: PathBuf,
    }

    #[fixture]
    pub(crate) fn layout() -> Layout {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let root = base.join("repo");
        fs::create_dir_all(root.join(".jj/repo")).unwrap();
        let added = root.join("workspaces/mine");
        fs::create_dir_all(added.join(".jj")).unwrap();
        fs::write(
            added.join(".jj/repo"),
            root.join(".jj/repo").as_os_str().as_encoded_bytes(),
        )
        .unwrap();
        let outside = base.join("outside");
        fs::create_dir_all(&outside).unwrap();
        Layout {
            _dir: dir,
            root,
            added,
            outside,
        }
    }

    #[rstest]
    fn default_workspace_includes_paths_that_do_not_exist_yet(layout: Layout) {
        let new_file = layout.root.join("src/new/module.rs");
        assert_eq!(default_workspace_root(&new_file), Some(layout.root.clone()));
        assert_eq!(default_workspace_root(&layout.root), Some(layout.root));
    }

    #[rstest]
    fn added_workspace_nested_in_default_is_not_default(layout: Layout) {
        assert_eq!(
            default_workspace_root(&layout.added.join("src/lib.rs")),
            None
        );
    }

    #[rstest]
    fn paths_outside_any_repo_are_not_default(layout: Layout) {
        assert_eq!(
            default_workspace_root(&layout.outside.join("notes.txt")),
            None
        );
        assert_eq!(default_workspace_root(Path::new("/dev/null")), None);
    }

    #[rstest]
    fn git_exclude_file_is_writable_but_not_the_rest_of_git(layout: Layout) {
        let info = layout.root.join(".git/info");
        std::fs::create_dir_all(&info).unwrap();
        assert_eq!(default_workspace_written(&info.join("exclude")), None);
        assert_eq!(
            default_workspace_written(&layout.root.join(".jj/repo/store/git/info/exclude")),
            None
        );
        assert_eq!(
            default_workspace_written(&info.join("attributes")),
            Some(layout.root.clone())
        );
        assert_eq!(
            default_workspace_written(&layout.root.join("src/lib.rs")),
            Some(layout.root)
        );
    }

    #[cfg(unix)]
    #[rstest]
    fn symlink_into_default_workspace_resolves_to_it(layout: Layout) {
        let link = layout.outside.join("link");
        std::os::unix::fs::symlink(&layout.root, &link).unwrap();
        assert_eq!(
            default_workspace_root(&link.join("src/main.rs")),
            Some(layout.root)
        );
    }

    #[rstest]
    #[case("/a/b", "c", "/a/b/c")]
    #[case("/a/b", "../c", "/a/c")]
    #[case("/a/b", "./c/./d", "/a/b/c/d")]
    #[case("/a/b", "/x/../y", "/y")]
    fn absolutize_joins_and_normalizes(
        #[case] base: &str,
        #[case] path: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(
            absolutize(Path::new(base), Path::new(path)),
            PathBuf::from(expected)
        );
    }
}
