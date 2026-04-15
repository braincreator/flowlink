//! Git rollback operations — undo commands by reverting to previous state
//!
//! Uses git2 for actual git operations. All rollback functions perform real
//! git operations and return accurate results.

use anyhow::{Context, Result};
use git2::{Commit, Repository, ResetType};
use tracing::{debug, info, warn};

use crate::config::GitConfig;

/// Rollback engine — reverts to previous commits
pub struct RollbackEngine {
    repo_path: std::path::PathBuf,
}

/// Result of a rollback operation
#[derive(Debug, Clone)]
pub struct RollbackResult {
    /// Commit hash we rolled back from
    pub from_commit: String,
    /// Commit hash we rolled back to
    pub to_commit: String,
    /// Files changed in the rollback
    pub files_changed: Vec<String>,
    /// Whether the rollback was successful
    pub success: bool,
}

impl RollbackEngine {
    pub fn new(config: GitConfig) -> Self {
        Self {
            repo_path: config.repo_path.into(),
        }
    }

    /// Open the git repository
    fn open_repo(&self) -> Result<Repository> {
        Repository::open(&self.repo_path)
            .with_context(|| format!("Failed to open repository at {:?}", self.repo_path))
    }

    /// Get HEAD commit, return error if HEAD is detached or unborn
    fn head_commit(repo: &Repository) -> Result<Commit<'_>> {
        let head = repo.head().context("Failed to resolve HEAD")?;
        if head.is_branch() {
            // Normal branch checkout
        }
        let target = head
            .target()
            .context("Cannot rollback: HEAD has no target (unborn branch)")?;
        let commit = repo
            .find_commit(target)
            .context("Failed to find HEAD commit")?;
        Ok(commit)
    }

    /// Get files changed between two commits
    fn diff_files(repo: &Repository, from: &git2::Oid, to: &git2::Oid) -> Result<Vec<String>> {
        let from_tree = repo.find_commit(*from)?.tree()?;
        let to_tree = repo.find_commit(*to)?.tree()?;
        let diff = repo
            .diff_tree_to_tree(Some(&from_tree), Some(&to_tree), None)
            .context("Failed to diff trees")?;

        let mut files = Vec::new();
        for delta in diff.deltas() {
            if let Some(path) = delta.new_file().path() {
                files.push(path.to_string_lossy().to_string());
            } else if let Some(path) = delta.old_file().path() {
                files.push(path.to_string_lossy().to_string());
            }
        }
        Ok(files)
    }

    /// Stash current working changes before a destructive rollback
    fn stash_current_changes(repo: &mut Repository) -> Result<Option<String>> {
        // Check if there are any changes to stash
        let has_changes = {
            let statuses = repo
                .statuses(Some(
                    git2::StatusOptions::new()
                        .include_untracked(true)
                        .recurse_untracked_dirs(true),
                ))
                .context("Failed to get repository status")?;

            statuses.iter().any(|s| {
                s.status().intersects(
                    git2::Status::INDEX_NEW
                        | git2::Status::INDEX_MODIFIED
                        | git2::Status::INDEX_DELETED
                        | git2::Status::INDEX_RENAMED
                        | git2::Status::INDEX_TYPECHANGE
                        | git2::Status::WT_MODIFIED
                        | git2::Status::WT_DELETED
                        | git2::Status::WT_RENAMED
                        | git2::Status::WT_NEW,
                )
            })
        };

        if !has_changes {
            return Ok(None);
        }

        // Add all changes to index for stashing
        let mut index = repo.index()?;
        index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
        index.write()?;

        let sig = repo
            .signature()
            .or_else(|_| git2::Signature::now("FlowLink", "flowlink@local"))?;

        let oid = repo
            .stash_save(&sig, "Auto-stash before rollback", None)
            .context("Failed to stash current changes")?;

        if oid.is_zero() {
            warn!("Stash save returned zero OID — nothing was stashed");
            Ok(None)
        } else {
            info!("Auto-stashed current changes: {}", oid);
            Ok(Some(oid.to_string()))
        }
    }

    /// Rollback to the previous commit (undo last) using `git reset --hard HEAD~1`
    pub async fn rollback_last(&self) -> Result<RollbackResult> {
        self.rollback_n(1).await
    }

    /// Rollback to a specific commit using `git reset --hard <hash>`
    pub async fn rollback_to(&self, commit_hash: &str) -> Result<RollbackResult> {
        info!("Rolling back to commit {}", commit_hash);

        let repo = self.open_repo()?;
        let head = Self::head_commit(&repo)?;
        let from_oid = head.id();
        let from_hex = from_oid.to_string();
        drop(head); // Release borrow on repo so we can mut-borrow for stash

        // Resolve target commit
        let to_oid = git2::Oid::from_str(commit_hash)
            .with_context(|| format!("Invalid commit hash: {}", commit_hash))?;

        // Get changed files before reset (immutable borrow)
        let files_changed = Self::diff_files(&repo, &from_oid, &to_oid)?;

        // Auto-stash current working changes (needs mutable borrow)
        let mut repo = repo;
        let _stash = Self::stash_current_changes(&mut repo)?;

        // Perform hard reset — find_object to avoid intermediate Commit borrow
        let target_object = repo
            .find_object(to_oid, Some(git2::ObjectType::Commit))
            .with_context(|| format!("Commit {} not found", commit_hash))?;
        repo.reset(&target_object, ResetType::Hard, None)
            .with_context(|| format!("Failed to reset to commit {}", commit_hash))?;

        info!(
            "Rolled back from {} to {} ({} files changed)",
            &from_hex[..8],
            commit_hash,
            files_changed.len()
        );

        Ok(RollbackResult {
            from_commit: from_hex,
            to_commit: commit_hash.to_string(),
            files_changed,
            success: true,
        })
    }

    /// Rollback by N commits using `git reset --hard HEAD~N`
    pub async fn rollback_n(&self, n: usize) -> Result<RollbackResult> {
        if n == 0 {
            anyhow::bail!("Cannot rollback 0 commits");
        }

        info!("Rolling back {} commits", n);

        let repo = self.open_repo()?;
        let head = Self::head_commit(&repo)?;
        let from_oid = head.id();
        let from_hex = from_oid.to_string();

        // Walk N commits back, collecting Oid (Copy type) to avoid holding borrow
        let mut to_oid = from_oid;
        {
            let mut current = head;
            for _ in 0..n {
                match current.parent(0) {
                    Ok(parent) => {
                        to_oid = parent.id();
                        current = parent;
                    }
                    Err(_) => {
                        anyhow::bail!(
                            "Cannot rollback {} commits: reached root commit {}",
                            n,
                            current.id()
                        );
                    }
                }
            }
            // current dropped here, releasing borrow on repo
        }
        let to_hex = to_oid.to_string();

        // Get changed files before reset (immutable borrow)
        let files_changed = Self::diff_files(&repo, &from_oid, &to_oid)?;

        // Auto-stash current working changes (needs mutable borrow)
        let mut repo = repo;
        let _stash = Self::stash_current_changes(&mut repo)?;

        // Perform hard reset
        let target_object = repo
            .find_object(to_oid, Some(git2::ObjectType::Commit))
            .context("Failed to find target commit object")?;
        repo.reset(&target_object, ResetType::Hard, None)
            .with_context(|| format!("Failed to reset to commit {}", to_hex))?;

        info!(
            "Rolled back {} commits: {} -> {} ({} files changed)",
            n,
            &from_hex[..8],
            &to_hex[..8],
            files_changed.len()
        );

        Ok(RollbackResult {
            from_commit: from_hex,
            to_commit: to_hex,
            files_changed,
            success: true,
        })
    }

    /// List recent commits for rollback selection using git2 revwalk
    pub async fn list_recent_commits(&self, count: usize) -> Result<Vec<CommitInfo>> {
        debug!("Listing {} recent commits", count);

        let repo = self.open_repo()?;
        let head = Self::head_commit(&repo)?;
        let _head_oid = head.id().to_string();

        let mut revwalk = repo.revwalk().context("Failed to create revwalk")?;
        revwalk.push_head()?;
        revwalk.simplify_first_parent()?;

        let mut commits = Vec::with_capacity(count);
        for oid in revwalk.take(count) {
            let oid = oid.context("Failed to read revwalk entry")?;
            let commit = repo.find_commit(oid)?;
            let hash = oid.to_string();
            let author = commit.author();
            let message = commit
                .message()
                .unwrap_or("(no message)")
                .lines()
                .next()
                .unwrap_or("(no message)")
                .to_string();

            commits.push(CommitInfo {
                hash: hash.clone(),
                short_hash: hash[..8.min(hash.len())].to_string(),
                message,
                author: format!(
                    "{} <{}>",
                    author.name().unwrap_or("Unknown"),
                    author.email().unwrap_or("unknown")
                ),
                timestamp: chrono::DateTime::from_timestamp(commit.time().seconds(), 0)
                    .unwrap_or_else(chrono::Utc::now),
                is_head: oid == head.id(),
            });
        }

        Ok(commits)
    }

    /// Create a revert commit (safe rollback — doesn't rewrite history)
    pub async fn revert_commit(&self, commit_hash: &str) -> Result<RollbackResult> {
        info!("Creating revert commit for {}", commit_hash);

        let repo = self.open_repo()?;
        let head = Self::head_commit(&repo)?;
        let from_oid = head.id();
        let from_hex = from_oid.to_string();

        // Resolve target commit
        let revert_oid = git2::Oid::from_str(commit_hash)
            .with_context(|| format!("Invalid commit hash: {}", commit_hash))?;
        let revert_commit = repo
            .find_commit(revert_oid)
            .with_context(|| format!("Commit {} not found", commit_hash))?;
        let head_commit = Self::head_commit(&repo)?;

        // Perform revert: revert_commit(repo, commit_to_revert, our_commit, mainline, options)
        let mut revert_index = repo
            .revert_commit(&revert_commit, &head_commit, 0u32, None)
            .context("Failed to revert commit — there may be conflicts")?;
        drop(head_commit);

        // If revert_index is empty, no changes were made (already reverted or empty diff)
        if revert_index.is_empty() {
            info!(
                "Revert of {} produced no changes (already reverted or empty)",
                commit_hash
            );
            return Ok(RollbackResult {
                from_commit: commit_hash.to_string(),
                to_commit: from_hex,
                files_changed: vec![],
                success: true,
            });
        }

        // Write the reverted index as a tree and create a new commit
        let tree_oid = revert_index
            .write_tree_to(&repo)
            .context("Failed to write reverted index to tree")?;
        let tree = repo
            .find_tree(tree_oid)
            .context("Failed to find reverted tree")?;

        let sig = repo
            .signature()
            .or_else(|_| git2::Signature::now("FlowLink", "flowlink@local"))?;
        let revert_msg = format!("Revert {}", commit_hash);
        let new_oid = repo
            .commit(
                Some("HEAD"),
                &sig,
                &sig,
                &revert_msg,
                &tree,
                &[&Self::head_commit(&repo)?],
            )
            .context("Failed to create revert commit")?;

        // Get the files changed by the revert
        let files_changed = Self::diff_files(&repo, &from_oid, &new_oid)?;

        info!(
            "Reverted commit {} -> {} ({} files changed)",
            commit_hash,
            &new_oid.to_string()[..8],
            files_changed.len()
        );

        Ok(RollbackResult {
            from_commit: commit_hash.to_string(),
            to_commit: new_oid.to_string(),
            files_changed,
            success: true,
        })
    }
}

/// Information about a git commit
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommitInfo {
    pub hash: String,
    pub short_hash: String,
    pub message: String,
    pub author: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub is_head: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SyncStrategy;
    use std::path::PathBuf;

    /// Helper: create a temp git repo with N commits, return (RollbackEngine, TempDir, Repository)
    /// TempDir must be held to prevent the directory from being deleted.
    fn setup_repo_with_commits(n: usize) -> (RollbackEngine, tempfile::TempDir, Repository) {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        // Configure repo identity
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();

        // Create initial empty commit
        {
            let tree_oid = {
                let mut builder = repo.treebuilder(None).unwrap();
                builder.write().unwrap()
            };
            let tree = repo.find_tree(tree_oid).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
                .unwrap();
        }

        // Add N more commits, each touching a different file
        for i in 1..=n {
            let file_name = format!("file_{}.txt", i);
            let blob_oid = repo.blob(format!("content {}", i).as_bytes()).unwrap();
            let parent = repo.head().unwrap().target().unwrap();
            let parent_commit = repo.find_commit(parent).unwrap();
            // Build tree based on parent's tree so we accumulate files
            let parent_tree = parent_commit.tree().unwrap();
            let tree_oid = {
                let mut builder = repo.treebuilder(Some(&parent_tree)).unwrap();
                builder.insert(&file_name, blob_oid, 0o100644).unwrap();
                builder.write().unwrap()
            };
            let tree = repo.find_tree(tree_oid).unwrap();
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                &format!("Commit {}", i),
                &tree,
                &[&parent_commit],
            )
            .unwrap();
        }

        // Checkout HEAD to populate working tree
        repo.checkout_head(None).unwrap();

        let config = GitConfig {
            repo_path: dir.path().to_string_lossy().to_string(),
            remote_url: None,
            branch: "main".to_string(),
            sync_strategy: SyncStrategy::Realtime,
            signing_key: None,
        };
        let engine = RollbackEngine::new(config);
        (engine, dir, repo)
    }

    #[tokio::test]
    async fn test_list_recent_commits() {
        let (engine, _dir, _repo) = setup_repo_with_commits(3);
        let commits = engine.list_recent_commits(10).await.unwrap();
        // Initial + 3 = 4 commits
        assert_eq!(commits.len(), 4);
        assert!(commits[0].is_head, "First commit should be HEAD");
        assert!(!commits[1].is_head);
        assert!(commits[0].hash.len() == 40, "Full hash should be 40 chars");
        assert!(
            commits[0].short_hash.len() == 8,
            "Short hash should be 8 chars"
        );
    }

    #[tokio::test]
    async fn test_list_recent_commits_limited() {
        let (engine, _dir, _repo) = setup_repo_with_commits(5);
        let commits = engine.list_recent_commits(2).await.unwrap();
        assert_eq!(commits.len(), 2);
    }

    #[tokio::test]
    async fn test_rollback_last() {
        let (engine, dir, repo) = setup_repo_with_commits(3);

        // Verify file_3.txt exists before rollback
        assert!(dir.path().join("file_3.txt").exists());

        let result = engine.rollback_last().await.unwrap();
        assert!(result.success);
        assert!(result.from_commit.len() == 40);
        assert!(result.to_commit.len() == 40);
        assert!(result.from_commit != result.to_commit);

        // After rollback_last (n=1), HEAD should match result.to_commit
        let new_repo = Repository::open(dir.path()).unwrap();
        let head = new_repo.head().unwrap().target().unwrap();
        assert_eq!(head.to_string(), result.to_commit);
    }

    #[tokio::test]
    async fn test_rollback_to_specific_commit() {
        let (engine, _dir, repo) = setup_repo_with_commits(3);

        // Get the second commit (Initial + 1)
        let head_before = repo.head().unwrap().target().unwrap();
        let parent = repo.find_commit(head_before).unwrap().parent(0).unwrap();
        let parent_of_parent = parent.parent(0).unwrap();
        let target_hash = parent_of_parent.id().to_string();

        let result = engine.rollback_to(&target_hash).await.unwrap();
        assert!(result.success);
        assert_eq!(result.to_commit, target_hash);

        // HEAD should now point to target
        let head_after = repo.head().unwrap().target().unwrap();
        assert_eq!(head_after.to_string(), target_hash);
    }

    #[tokio::test]
    async fn test_rollback_n() {
        let (engine, _dir, repo) = setup_repo_with_commits(5);

        let head_before = repo.head().unwrap().target().unwrap();
        let result = engine.rollback_n(2).await.unwrap();
        assert!(result.success);

        // HEAD should have moved back 2 commits
        let head_after = repo.head().unwrap().target().unwrap();
        assert_ne!(head_before, head_after);

        // Walk back 2 from original HEAD to verify
        let original = repo.find_commit(head_before).unwrap();
        let expected = original.parent(0).unwrap().parent(0).unwrap();
        assert_eq!(head_after, expected.id());
    }

    #[tokio::test]
    async fn test_rollback_n_zero_fails() {
        let (engine, _dir, _repo) = setup_repo_with_commits(2);
        let result = engine.rollback_n(0).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_rollback_n_too_many_fails() {
        let (engine, _dir, _repo) = setup_repo_with_commits(1);
        // Only 2 commits total (initial + 1), can't go back 5
        let result = engine.rollback_n(5).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_rollback_invalid_hash_fails() {
        let (engine, _dir, _repo) = setup_repo_with_commits(1);
        let result = engine.rollback_to("not-a-valid-hash").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_revert_commit() {
        let (engine, dir, _repo) = setup_repo_with_commits(3);

        // Get the last commit hash from the engine's perspective
        let commits = engine.list_recent_commits(1).await.unwrap();
        let last_hash = commits[0].hash.clone();

        // Verify file exists
        assert!(dir.path().join("file_3.txt").exists());

        // Revert should create a new commit undoing the last
        let result = engine.revert_commit(&last_hash).await.unwrap();
        assert!(result.success);
        assert_ne!(
            result.from_commit, result.to_commit,
            "Revert should create a new commit"
        );
    }

    #[tokio::test]
    async fn test_files_changed_populated() {
        let (engine, _dir, _repo) = setup_repo_with_commits(3);
        let result = engine.rollback_last().await.unwrap();
        // rollback_last removes file_3.txt
        assert!(result.files_changed.contains(&"file_3.txt".to_string()));
    }
}
