//! Pure staleness reporter for jj change descriptions.
//!
//! Takes a revset as its sole argument, evaluates it via `jj log`, then uses
//! jj-lib for in-memory evolog walks and tree diffs. Prints a plain-text
//! staleness report to stdout if any changes have stale descriptions.
//!
//! Exit codes:
//! - 0: success (stdout may contain a report, or be empty if all clean)
//! - 1: revset evaluation failed (`jj log` errored)
//! - Other non-zero: unexpected error (fail-open from `main()`)

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use futures::StreamExt as _;
use jj_lib::backend::{CommitId, MergedTreeValue};
use jj_lib::commit::Commit;
use jj_lib::config::StackedConfig;
use jj_lib::default_backend_factories::default_backend_factories;
use jj_lib::evolution::walk_predecessors;
use jj_lib::matchers::EverythingMatcher;
use jj_lib::merge::Diff;
use jj_lib::repo::{ReadonlyRepo, Repo as _, RepoLoader};
use jj_lib::repo_path::RepoPathBuf;
use jj_lib::settings::UserSettings;
use pollster::FutureExt as _;

/// Maximum evolog entries to inspect per change (sanity bound).
const MAX_EVOLOG_ENTRIES: usize = 200;

/// Per-change staleness message length (in bytes) above which the full detail
/// is spilled to a temp file and only the path is printed inline — analogous to
/// how rustc spills very long diagnostics.
const STALENESS_SPILL_THRESHOLD: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq)]
struct StalenessInfo {
    change_id_short: String,
    /// Files whose diff-from-parent changed since the last describe.
    changed_files: Vec<RepoPathBuf>,
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    // `ack` is run by an agent, so its errors are reported, not swallowed.
    if args.first().map(String::as_str) == Some("ack") {
        let revset = args.get(1).map_or("@", String::as_str);
        if let Err(e) = acknowledge(revset) {
            #[allow(clippy::print_stderr)]
            {
                eprintln!("active-descriptions ack: {e:#}");
            }
            std::process::exit(1);
        }
        return;
    }

    // Fail open: any error → exit 0 so we never block Claude.
    if let Err(e) = run(&args) {
        // Only surface errors when debugging.
        if env::var_os("ACTIVE_DESCRIPTIONS_DEBUG").is_some() {
            #[allow(clippy::print_stderr)]
            {
                eprintln!("active-descriptions: {e:#}");
            }
        }
    }
}

fn run(args: &[String]) -> Result<()> {
    let revset = args
        .first()
        .context("usage: jj-stale-descriptions [ack] <revset>")?;

    // Gather candidate commit IDs via subprocess (evaluates revset with full
    // CLI context, triggers working-copy snapshot).
    let candidate_hex = gather_candidates(revset).map_err(|e| {
        // Exit 1 signals revset failure to the shell wrapper.
        if env::var_os("ACTIVE_DESCRIPTIONS_DEBUG").is_some() {
            #[allow(clippy::print_stderr)]
            {
                eprintln!("active-descriptions: {e:#}");
            }
        }
        std::process::exit(1);
    })?;

    if candidate_hex.is_empty() {
        return Ok(());
    }

    // Load repo via jj-lib.
    let (repo, repo_path) = load_repo()?;
    let acks = Acks::load(&repo_path)?;

    // Check each candidate for staleness.
    let mut stale: Vec<StalenessInfo> = Vec::new();
    for hex in &candidate_hex {
        let commit_id = CommitId::try_from_hex(hex.as_bytes())
            .with_context(|| format!("invalid commit id hex: {hex}"))?;
        if let Some(info) = check_staleness(&repo, &commit_id, &acks)? {
            stale.push(info);
        }
    }

    stale.dedup_by(|a, b| a.change_id_short == b.change_id_short);

    if !stale.is_empty() {
        let msg = format_staleness_message(&stale);
        #[allow(clippy::print_stdout)]
        {
            print!("{msg}");
        }
    }

    Ok(())
}

/// Records that each change in `revset` still matches its description,
/// although its diff changed since it was described (for example after
/// `jj fix` or regenerating code), so the stale check accepts it until its
/// diff changes again.
fn acknowledge(revset: &str) -> Result<()> {
    let candidate_hex = gather_candidates(revset)?;
    let (repo, repo_path) = load_repo()?;
    let mut acks = Acks::load(&repo_path)?;
    let mut acknowledged = Vec::new();
    for hex in &candidate_hex {
        let commit_id = CommitId::try_from_hex(hex.as_bytes())
            .with_context(|| format!("invalid commit id hex: {hex}"))?;
        let commit = repo.store().get_commit(&commit_id)?;
        // An empty description is always stale; there is nothing to confirm.
        if commit.description().is_empty() {
            continue;
        }
        let change_id = commit.change_id().to_string();
        acks.record(&change_id, &commit_diff_fingerprint(&repo, &commit)?);
        acknowledged.push(change_id[..change_id.len().min(12)].to_owned());
    }
    acks.save()?;
    #[allow(clippy::print_stdout)]
    {
        println!("Acknowledged {} change(s): {}", acknowledged.len(), acknowledged.join(", "));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Acknowledgements
// ---------------------------------------------------------------------------

/// A change's diff from its parents, keyed by path.
type Fingerprint = BTreeMap<RepoPathBuf, Diff<MergedTreeValue>>;

/// Changes whose current diff an agent has confirmed still matches their
/// description: full change ID → hash of that diff. Kept in the repo store,
/// so every workspace shares it and it is never committed.
struct Acks {
    path: PathBuf,
    entries: BTreeMap<String, String>,
}

impl Acks {
    fn load(repo_path: &Path) -> Result<Self> {
        let path = repo_path.join("active-descriptions-acks");
        let entries = match fs::read_to_string(&path) {
            Ok(text) => text
                .lines()
                .filter_map(|line| line.split_once(' '))
                .map(|(change, hash)| (change.to_owned(), hash.to_owned()))
                .collect(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(e) => return Err(e).with_context(|| format!("failed to read {}", path.display())),
        };
        Ok(Self { path, entries })
    }

    fn covers(&self, change_id: &str, fingerprint: &Fingerprint) -> bool {
        self.entries.get(change_id) == Some(&fingerprint_hash(fingerprint))
    }

    fn record(&mut self, change_id: &str, fingerprint: &Fingerprint) {
        self.entries
            .insert(change_id.to_owned(), fingerprint_hash(fingerprint));
    }

    /// Writes through a temporary file so concurrent readers never see a
    /// partial file.
    fn save(&self) -> Result<()> {
        let text: String = self
            .entries
            .iter()
            .map(|(change, hash)| format!("{change} {hash}\n"))
            .collect();
        let temp = self.path.with_extension("tmp");
        fs::write(&temp, text).with_context(|| format!("failed to write {}", temp.display()))?;
        fs::rename(&temp, &self.path)
            .with_context(|| format!("failed to replace {}", self.path.display()))
    }
}

fn fingerprint_hash(fingerprint: &Fingerprint) -> String {
    use sha2::{Digest as _, Sha256};
    let digest = Sha256::digest(format!("{fingerprint:?}").as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

// ---------------------------------------------------------------------------
// Subprocess: gather candidate commit IDs
// ---------------------------------------------------------------------------

/// Runs `jj log` to evaluate the given revset and return full hex commit IDs.
fn gather_candidates(revset: &str) -> Result<Vec<String>> {
    let output = Command::new("jj")
        .args(["log", "-r", revset, "--no-graph", "-T", r#"commit_id ++ "\n""#])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .context("failed to run `jj log`")?;

    if !output.status.success() {
        bail!("jj log failed for revset: {revset}");
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect())
}

// ---------------------------------------------------------------------------
// jj-lib repo loading
// ---------------------------------------------------------------------------

/// Loads the repo at HEAD. Discovers the workspace root from `jj root`, then
/// initializes a `RepoLoader` from the `.jj/repo` path.
fn load_repo() -> Result<(Arc<ReadonlyRepo>, PathBuf)> {
    let workspace_root = discover_workspace_root()?;
    let repo_path = resolve_repo_path(&workspace_root.join(".jj").join("repo"))?;

    let config = StackedConfig::with_defaults();
    let settings =
        UserSettings::from_config(config).context("failed to create UserSettings from defaults")?;
    let store_factories = default_backend_factories();

    let loader = RepoLoader::init_from_file_system(&settings, &repo_path, &store_factories)
        .context("failed to init repo loader")?;
    let repo = loader
        .load_at_head()
        .block_on()
        .context("failed to load repo at head")?;

    Ok((repo, repo_path))
}

/// Resolves the repo path, following jj's workspace indirection.
///
/// In secondary workspaces, `.jj/repo` is a file containing the path to the
/// primary workspace's repo directory rather than a directory itself. jj
/// writes that path relative to the `.jj` directory holding the pointer.
fn resolve_repo_path(path: &Path) -> Result<PathBuf> {
    if path.is_file() {
        let target = fs::read_to_string(path)
            .with_context(|| format!("failed to read repo pointer at {}", path.display()))?;
        let dot_jj = path
            .parent()
            .with_context(|| format!("repo pointer {} has no parent", path.display()))?;
        Ok(dot_jj.join(target.trim()))
    } else {
        Ok(path.to_path_buf())
    }
}

/// Gets the workspace root by running `jj root`.
fn discover_workspace_root() -> Result<PathBuf> {
    let output = Command::new("jj")
        .args(["root"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .context("failed to run `jj root`")?;

    if !output.status.success() {
        bail!("not a jj repo (jj root failed)");
    }

    let root = String::from_utf8(output.stdout)
        .context("jj root output is not utf-8")?
        .trim()
        .to_owned();

    Ok(PathBuf::from(root))
}

// ---------------------------------------------------------------------------
// Staleness detection
// ---------------------------------------------------------------------------

/// Checks whether a commit's description is stale relative to its content.
///
/// Returns `None` when the description is current, or `Some(StalenessInfo)`
/// with the change ID and list of files whose diff-from-parent changed since
/// the description was last set.
///
/// A description is stale if:
/// - The commit has a non-empty diff but an empty description, OR
/// - The commit's diff-from-parent has changed since the description was last
///   set (determined by walking the evolution log and comparing tree diffs).
///
/// This compares actual diffs rather than using heuristics about tree/parent
/// change ordering, which avoids false positives from splits, squashes, and
/// rebases that alter the tree without changing the logical content.
fn check_staleness(
    repo: &ReadonlyRepo,
    commit_id: &CommitId,
    acks: &Acks,
) -> Result<Option<StalenessInfo>> {
    let commit = repo.store().get_commit(commit_id)?;

    // ChangeId::Display uses reverse_hex (the user-facing jj format).
    let full_change_id = commit.change_id().to_string();
    let change_id_short = full_change_id[..full_change_id.len().min(12)].to_owned();

    // Empty description on a non-empty change is always stale.
    // Report every file in the current diff as changed.
    if commit.description().is_empty() {
        let current_diff = commit_diff_fingerprint(repo, &commit)?;
        let changed_files: Vec<RepoPathBuf> = current_diff.into_keys().collect();
        return Ok(Some(StalenessInfo {
            change_id_short,
            changed_files,
        }));
    }

    // Collect evolution entries (newest first from walk_predecessors, so we
    // reverse to get chronological order).
    let mut entries = Vec::new();
    let mut evolution = std::pin::pin!(walk_predecessors(repo, std::slice::from_ref(commit_id)));
    while let Some(result) = evolution.next().block_on() {
        let entry = result.context("evolog walk failed")?;
        entries.push(entry);
        if entries.len() >= MAX_EVOLOG_ENTRIES {
            break;
        }
    }

    // walk_predecessors yields newest-first; reverse for chronological.
    entries.reverse();

    if entries.len() < 2 {
        // Single entry (freshly created) — if it has a description, it's fine.
        return Ok(None);
    }

    // Find the evolog entry where the description was last changed.
    let mut last_described_commit: Option<&Commit> = None;
    for i in (1..entries.len()).rev() {
        if entries[i].commit.description() != entries[i - 1].commit.description() {
            last_described_commit = Some(&entries[i].commit);
            break;
        }
    }

    // If the description was never changed, it was established at the first
    // evolog entry. We still need to compare its diff to the current diff to
    // catch content edits that happened after the initial describe.
    let described_commit = last_described_commit.unwrap_or(&entries[0].commit);

    // Compare the diff-from-parent at describe-time vs now. If identical,
    // the logical content hasn't changed and the description is still valid.
    let described_diff = commit_diff_fingerprint(repo, described_commit)?;
    let current_diff = commit_diff_fingerprint(repo, &commit)?;

    if described_diff == current_diff || acks.covers(&full_change_id, &current_diff) {
        return Ok(None);
    }

    let changed_files = diff_fingerprint_changes(&described_diff, &current_diff);

    Ok(Some(StalenessInfo {
        change_id_short,
        changed_files,
    }))
}

/// Computes a fingerprint of a commit's diff from its parent(s).
///
/// Returns a sorted map of `(path → (before, after))` tree value pairs. Two
/// commits have the same logical content iff their fingerprints are equal,
/// regardless of what parents they sit on.
fn commit_diff_fingerprint(
    repo: &ReadonlyRepo,
    commit: &Commit,
) -> Result<BTreeMap<RepoPathBuf, Diff<MergedTreeValue>>> {
    let tree = commit.tree();
    let parent_tree = commit.parent_tree(repo).block_on()?;

    let mut fingerprint = BTreeMap::new();
    let mut stream = parent_tree.diff_stream(&tree, &EverythingMatcher);

    async {
        while let Some(entry) = stream.next().await {
            let diff = entry.values?;
            fingerprint.insert(entry.path, diff);
        }
        anyhow::Ok(())
    }
    .block_on()?;

    Ok(fingerprint)
}

/// Returns the set of paths whose diff-from-parent entry differs between two
/// fingerprints. This is the set of files that "changed" between two points
/// in a commit's evolution.
fn diff_fingerprint_changes(
    described: &BTreeMap<RepoPathBuf, Diff<MergedTreeValue>>,
    current: &BTreeMap<RepoPathBuf, Diff<MergedTreeValue>>,
) -> Vec<RepoPathBuf> {
    let mut changed = Vec::new();

    // Paths present in current but absent or different in described.
    for (path, cur_diff) in current {
        match described.get(path) {
            Some(desc_diff) if desc_diff == cur_diff => {}
            _ => changed.push(path.clone()),
        }
    }

    // Paths removed from the diff (present in described, absent in current).
    for path in described.keys() {
        if !current.contains_key(path) {
            changed.push(path.clone());
        }
    }

    changed.sort();
    changed
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

/// Formats the staleness detail for a single change.
fn format_single_staleness(info: &StalenessInfo) -> String {
    use std::fmt::Write as _;

    let mut msg = format!(
        "Stale description: change {} modified since last described.",
        info.change_id_short
    );
    if !info.changed_files.is_empty() {
        let files: Vec<_> = info
            .changed_files
            .iter()
            .map(|f| f.as_internal_file_string().to_owned())
            .collect();
        let _ = write!(msg, "\n  Changed: {}", files.join(", "));
    }
    msg
}

/// Builds a human-readable staleness summary including changed file paths.
///
/// When a single change's message exceeds [`STALENESS_SPILL_THRESHOLD`], the
/// full detail is written to a temp file and only the path is printed inline.
fn format_staleness_message(stale: &[StalenessInfo]) -> String {
    let mut msg = String::new();
    for (i, info) in stale.iter().enumerate() {
        if i > 0 {
            msg.push('\n');
        }

        let detail = format_single_staleness(info);

        if detail.len() > STALENESS_SPILL_THRESHOLD {
            match spill_to_tempfile(&info.change_id_short, &detail) {
                Ok(path) => {
                    msg.push_str(&format!(
                        "Stale description: change {} modified since last described \
                         (full detail: {})",
                        info.change_id_short,
                        path.display(),
                    ));
                }
                Err(_) => {
                    // Spill failed — fall back to inline.
                    msg.push_str(&detail);
                }
            }
        } else {
            msg.push_str(&detail);
        }
    }
    msg
}

/// Writes the full staleness detail for a change to a persistent temp file.
fn spill_to_tempfile(change_id_short: &str, detail: &str) -> Result<PathBuf> {
    let file = tempfile::Builder::new()
        .prefix(&format!("stale-desc-{change_id_short}-"))
        .suffix(".txt")
        .tempfile()
        .context("failed to create staleness detail tempfile")?;

    let (_persisted, path) = file
        .keep()
        .map_err(|e| anyhow::anyhow!("failed to persist tempfile: {e}"))?;

    fs::write(&path, detail).with_context(|| {
        format!(
            "failed to write staleness detail to {}",
            path.display()
        )
    })?;

    Ok(path)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use testutils::{TestRepo, create_tree};

    /// Helper: create a tree with the given file contents.
    fn tree(
        repo: &Arc<ReadonlyRepo>,
        files: &[(&str, &str)],
    ) -> jj_lib::merged_tree::MergedTree {
        let path_contents: Vec<_> = files
            .iter()
            .map(|(p, c)| {
                (
                    jj_lib::repo_path::RepoPath::from_internal_string(p)
                        .expect("valid path"),
                    *c,
                )
            })
            .collect();
        create_tree(repo, &path_contents)
    }

    #[test]
    fn repo_pointer_resolves_relative_to_its_dot_jj() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = dir.path().join("project/.jj/repo");
        let pointer = dir.path().join("project/workspaces/mine/.jj/repo");
        fs::create_dir_all(&store).expect("store dir");
        fs::create_dir_all(pointer.parent().expect("pointer parent")).expect("pointer dir");
        // The form jj writes for a workspace nested in the default one.
        fs::write(&pointer, "../../../.jj/repo").expect("pointer");

        let resolved = resolve_repo_path(&pointer).expect("resolve");
        assert_eq!(
            resolved.canonicalize().expect("exists"),
            store.canonicalize().expect("exists")
        );
        assert_eq!(resolve_repo_path(&store).expect("resolve"), store);
    }

    #[test]
    fn empty_description_is_stale() {
        let test_repo = TestRepo::init();
        let repo = &test_repo.repo;

        let t = tree(repo, &[("file.txt", "content")]);
        let mut tx = repo.start_transaction();
        let commit = tx
            .repo_mut()
            .new_commit(vec![repo.store().root_commit_id().clone()], t)
            .write()
            .block_on()
            .expect("write commit");
        let repo = tx.commit("create").block_on().expect("commit tx");

        assert!(check_staleness(&repo, commit.id(), &no_acks())
            .expect("check_staleness")
            .is_some());
    }

    #[test]
    fn described_at_creation_not_stale() {
        let test_repo = TestRepo::init();
        let repo = &test_repo.repo;

        let t = tree(repo, &[("file.txt", "content")]);
        let mut tx = repo.start_transaction();
        let commit = tx
            .repo_mut()
            .new_commit(vec![repo.store().root_commit_id().clone()], t)
            .set_description("feat: add file")
            .write()
            .block_on()
            .expect("write commit");
        let repo = tx.commit("create").block_on().expect("commit tx");

        assert!(check_staleness(&repo, commit.id(), &no_acks())
            .expect("check_staleness")
            .is_none());
    }

    #[test]
    fn acknowledged_edit_is_not_stale_until_the_diff_changes_again() {
        let test_repo = TestRepo::init();
        let repo = &test_repo.repo;
        let dir = tempfile::tempdir().expect("tempdir");

        // Describe, then change content the way `jj fix` would.
        let mut tx = repo.start_transaction();
        let c1 = tx
            .repo_mut()
            .new_commit(vec![repo.store().root_commit_id().clone()], tree(repo, &[("file.txt", "v1")]))
            .set_description("feat: initial")
            .write()
            .block_on()
            .expect("write");
        let repo = tx.commit("create").block_on().expect("tx");
        let mut tx = repo.start_transaction();
        let c2 = tx
            .repo_mut()
            .rewrite_commit(&c1)
            .set_tree(tree(&repo, &[("file.txt", "v1 reformatted")]))
            .write()
            .block_on()
            .expect("rewrite");
        tx.repo_mut().rebase_descendants().block_on().expect("rebase descendants");
        let repo = tx.commit("fix").block_on().expect("tx");

        // Acknowledging the current diff clears it, across a save and reload.
        let mut acks = Acks::load(dir.path()).expect("load");
        let change_id = c2.change_id().to_string();
        acks.record(&change_id, &commit_diff_fingerprint(&repo, &c2).expect("fingerprint"));
        acks.save().expect("save");
        let acks = Acks::load(dir.path()).expect("reload");
        assert!(check_staleness(&repo, c2.id(), &acks).expect("check").is_none());

        // A later content change is stale again.
        let mut tx = repo.start_transaction();
        let c3 = tx
            .repo_mut()
            .rewrite_commit(&c2)
            .set_tree(tree(&repo, &[("file.txt", "v2")]))
            .write()
            .block_on()
            .expect("rewrite");
        tx.repo_mut().rebase_descendants().block_on().expect("rebase descendants");
        let repo = tx.commit("edit").block_on().expect("tx");
        assert!(check_staleness(&repo, c3.id(), &acks).expect("check").is_some());
    }

    fn no_acks() -> Acks {
        Acks {
            path: PathBuf::from("/nonexistent/active-descriptions-acks"),
            entries: BTreeMap::new(),
        }
    }

    #[test]
    fn content_edit_after_describe_is_stale() {
        let test_repo = TestRepo::init();
        let repo = &test_repo.repo;

        // Create commit with content + description.
        let t = tree(repo, &[("file.txt", "v1")]);
        let mut tx = repo.start_transaction();
        let c1 = tx
            .repo_mut()
            .new_commit(vec![repo.store().root_commit_id().clone()], t)
            .set_description("feat: initial")
            .write()
            .block_on()
            .expect("write");
        let repo = tx.commit("create").block_on().expect("tx");

        // Edit content without updating description.
        let t2 = tree(&repo, &[("file.txt", "v2")]);
        let mut tx = repo.start_transaction();
        let c2 = tx
            .repo_mut()
            .rewrite_commit(&c1)
            .set_tree(t2)
            .write()
            .block_on()
            .expect("rewrite");
        tx.repo_mut().rebase_descendants().block_on().expect("rebase descendants");
        let repo = tx.commit("edit").block_on().expect("tx");

        let info = check_staleness(&repo, c2.id(), &no_acks())
            .expect("check_staleness")
            .expect("should be stale");
        assert_eq!(
            info.changed_files.iter().map(|f| f.as_internal_file_string().to_owned()).collect::<Vec<_>>(),
            vec!["file.txt"],
        );
    }

    #[test]
    fn describe_after_content_edit_not_stale() {
        let test_repo = TestRepo::init();
        let repo = &test_repo.repo;

        // Create commit with content.
        let t = tree(repo, &[("file.txt", "v1")]);
        let mut tx = repo.start_transaction();
        let c1 = tx
            .repo_mut()
            .new_commit(vec![repo.store().root_commit_id().clone()], t)
            .set_description("feat: initial")
            .write()
            .block_on()
            .expect("write");
        let repo = tx.commit("create").block_on().expect("tx");

        // Edit content.
        let t2 = tree(&repo, &[("file.txt", "v2")]);
        let mut tx = repo.start_transaction();
        let c2 = tx
            .repo_mut()
            .rewrite_commit(&c1)
            .set_tree(t2)
            .write()
            .block_on()
            .expect("rewrite");
        tx.repo_mut().rebase_descendants().block_on().expect("rebase descendants");
        let repo = tx.commit("edit content").block_on().expect("tx");

        // Update description to match.
        let mut tx = repo.start_transaction();
        let c3 = tx
            .repo_mut()
            .rewrite_commit(&c2)
            .set_description("feat: updated")
            .write()
            .block_on()
            .expect("describe");
        tx.repo_mut().rebase_descendants().block_on().expect("rebase descendants");
        let repo = tx.commit("describe").block_on().expect("tx");

        assert!(check_staleness(&repo, c3.id(), &no_acks())
            .expect("check_staleness")
            .is_none());
    }

    #[test]
    fn rebase_without_content_change_not_stale() {
        let test_repo = TestRepo::init();
        let repo = &test_repo.repo;
        let root_id = repo.store().root_commit_id().clone();

        // Create parent commit.
        let parent_tree = tree(repo, &[("base.txt", "base")]);
        let mut tx = repo.start_transaction();
        let parent = tx
            .repo_mut()
            .new_commit(vec![root_id.clone()], parent_tree)
            .set_description("base")
            .write()
            .block_on()
            .expect("write parent");
        let repo = tx.commit("create parent").block_on().expect("tx");

        // Create child commit with description.
        let child_tree = tree(&repo, &[("base.txt", "base"), ("feat.txt", "feature")]);
        let mut tx = repo.start_transaction();
        let child = tx
            .repo_mut()
            .new_commit(vec![parent.id().clone()], child_tree)
            .set_description("feat: add feature")
            .write()
            .block_on()
            .expect("write child");
        let repo = tx.commit("create child").block_on().expect("tx");

        // Simulate rebase: change parent but keep same diff (feat.txt added).
        let new_parent_tree =
            tree(&repo, &[("base.txt", "base"), ("other.txt", "other")]);
        let mut tx = repo.start_transaction();
        let new_parent = tx
            .repo_mut()
            .new_commit(vec![root_id.clone()], new_parent_tree)
            .set_description("base v2")
            .write()
            .block_on()
            .expect("write new parent");

        // Rebased child: new parent, but still adds feat.txt.
        let rebased_tree = tree(
            tx.repo().base_repo(),
            &[
                ("base.txt", "base"),
                ("other.txt", "other"),
                ("feat.txt", "feature"),
            ],
        );
        let rebased = tx
            .repo_mut()
            .rewrite_commit(&child)
            .set_parents(vec![new_parent.id().clone()])
            .set_tree(rebased_tree)
            .write()
            .block_on()
            .expect("rebase");
        tx.repo_mut().rebase_descendants().block_on().expect("rebase descendants");
        let repo = tx.commit("rebase").block_on().expect("tx");

        // Diff is still just "add feat.txt" → not stale.
        assert!(check_staleness(&repo, rebased.id(), &no_acks())
            .expect("check_staleness")
            .is_none());
    }

    #[test]
    fn split_preserving_diff_not_stale() {
        let test_repo = TestRepo::init();
        let repo = &test_repo.repo;
        let root_id = repo.store().root_commit_id().clone();

        // Create commit with two files and a description.
        let t = tree(repo, &[("a.txt", "aaa"), ("b.txt", "bbb")]);
        let mut tx = repo.start_transaction();
        let original = tx
            .repo_mut()
            .new_commit(vec![root_id.clone()], t)
            .set_description("feat: add a and b")
            .write()
            .block_on()
            .expect("write");
        let repo = tx.commit("create").block_on().expect("tx");

        // Simulate split: first commit gets b.txt, second (rewrite of
        // original) gets a.txt but is reparented onto first.
        let first_tree = tree(&repo, &[("b.txt", "bbb")]);
        let mut tx = repo.start_transaction();
        let first = tx
            .repo_mut()
            .new_commit(vec![root_id.clone()], first_tree)
            .set_description("feat: add b")
            .write()
            .block_on()
            .expect("write first");

        // Remaining commit: reparented, full tree includes parent's b.txt.
        let remaining_tree =
            tree(tx.repo().base_repo(), &[("a.txt", "aaa"), ("b.txt", "bbb")]);
        let remaining = tx
            .repo_mut()
            .rewrite_commit(&original)
            .set_parents(vec![first.id().clone()])
            .set_tree(remaining_tree)
            .set_description("feat: add a")
            .write()
            .block_on()
            .expect("write remaining");
        tx.repo_mut().rebase_descendants().block_on().expect("rebase descendants");
        let repo = tx.commit("split").block_on().expect("tx");

        // The remaining commit's diff is "add a.txt", and its description
        // was set in the same operation. Not stale.
        assert!(check_staleness(&repo, remaining.id(), &no_acks())
            .expect("check_staleness")
            .is_none());
    }

    #[test]
    fn squash_changing_diff_is_stale() {
        let test_repo = TestRepo::init();
        let repo = &test_repo.repo;
        let root_id = repo.store().root_commit_id().clone();

        // Create commit with description covering one file.
        let t = tree(repo, &[("original.txt", "content")]);
        let mut tx = repo.start_transaction();
        let c1 = tx
            .repo_mut()
            .new_commit(vec![root_id.clone()], t)
            .set_description("feat: add original")
            .write()
            .block_on()
            .expect("write");
        let repo = tx.commit("create").block_on().expect("tx");

        // Squash new content in without updating description.
        let t2 = tree(&repo, &[("original.txt", "content"), ("extra.txt", "extra")]);
        let mut tx = repo.start_transaction();
        let c2 = tx
            .repo_mut()
            .rewrite_commit(&c1)
            .set_tree(t2)
            .write()
            .block_on()
            .expect("squash");
        tx.repo_mut().rebase_descendants().block_on().expect("rebase descendants");
        let repo = tx.commit("squash").block_on().expect("tx");

        // Diff changed (now includes extra.txt) but description wasn't updated.
        let info = check_staleness(&repo, c2.id(), &no_acks())
            .expect("check_staleness")
            .expect("should be stale");
        assert_eq!(
            info.changed_files.iter().map(|f| f.as_internal_file_string().to_owned()).collect::<Vec<_>>(),
            vec!["extra.txt"],
        );
    }
}
