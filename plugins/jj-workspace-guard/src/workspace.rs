//! Finding where a path is: which jj workspace, if any, owns it.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock, PoisonError};

/// Where a path is, as far as the guard is concerned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Zone {
    /// Inside the guarded default workspace with this root: read-only for
    /// agents.
    Default(PathBuf),
    /// Outside every jj repo, where project work does not belong.
    Outside,
    /// Anywhere else: an added workspace, or a repo that opted out.
    Free,
}

/// Returns the zone `path` is in.
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
/// never guarded: that is for repos that are not projects, such as a
/// dotfiles source or a scratch repo.
pub fn zone(path: &Path) -> Zone {
    let resolved = resolve_existing_prefix(path);
    let Some(workspace_root) = resolved.ancestors().find(|dir| dir.join(".jj").is_dir()) else {
        return Zone::Outside;
    };
    if workspace_root.join(".jj/repo").is_dir() && guarded(workspace_root) {
        Zone::Default(workspace_root.to_path_buf())
    } else {
        Zone::Free
    }
}

/// Returns the root of the guarded default workspace containing `path`, if
/// any.
pub fn default_workspace_root(path: &Path) -> Option<PathBuf> {
    match zone(path) {
        Zone::Default(root) => Some(root),
        Zone::Outside | Zone::Free => None,
    }
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

/// Returns the zone a write to `path` lands in, counting allowed writes as
/// [`Zone::Free`].
///
/// That is [`zone`], with three exceptions:
/// - Claude Code's per-project memory directory is where Claude Code tells
///   agents to write, wherever they work.
/// - Outside every repo, temporary folders (including Claude's session
///   scratchpad) hold throwaway files, not project work. A repo that happens
///   to live in one keeps its own rules.
/// - In a default workspace, the repo's git exclude file is local, untracked
///   configuration, and an agent there needs to exclude the `workspaces/`
///   directory it is about to create.
pub fn write_zone(path: &Path) -> Zone {
    if std::env::var_os("HOME").is_some_and(|home| is_claude_memory(path, Path::new(&home))) {
        return Zone::Free;
    }
    match zone(path) {
        Zone::Outside if is_temporary(path) || path.starts_with("/dev") => Zone::Free,
        Zone::Default(root) => {
            let resolved = resolve_existing_prefix(path);
            let exclude_files = [".git/info/exclude", ".jj/repo/store/git/info/exclude"];
            if exclude_files.iter().any(|file| resolved == root.join(file)) {
                Zone::Free
            } else {
                Zone::Default(root)
            }
        }
        other => other,
    }
}

/// Whether `path` is in a system temporary folder.
fn is_temporary(path: &Path) -> bool {
    let resolved = resolve_existing_prefix(path);
    let mut temporary = vec![
        PathBuf::from("/tmp"),
        PathBuf::from("/var/tmp"),
        PathBuf::from("/private/tmp"),
        PathBuf::from("/private/var/tmp"),
        resolve_existing_prefix(&std::env::temp_dir()),
    ];
    temporary.dedup();
    temporary.iter().any(|dir| resolved.starts_with(dir))
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

    use super::{Zone, absolutize, default_workspace_root, is_claude_memory, write_zone};

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
        assert_eq!(write_zone(&info.join("exclude")), Zone::Free);
        assert_eq!(write_zone(&layout.root.join(".jj/repo/store/git/info/exclude")), Zone::Free);
        assert_eq!(write_zone(&info.join("attributes")), Zone::Default(layout.root.clone()));
        assert_eq!(write_zone(&layout.root.join("src/lib.rs")), Zone::Default(layout.root));
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
