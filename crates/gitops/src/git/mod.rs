use crate::{
    config::{ConfigError, GitConfig},
    types::*,
};
use anyhow::{Context, Result};
use git2::{Oid, Repository, Signature, StatusOptions};
use serde_json::Value;
use std::path::PathBuf;
use tokio::task::spawn_blocking;
use tracing::{debug, info};

pub mod commit;
pub mod repo;
pub mod rollback;
pub mod sync;

pub use rollback::RollbackEngine;
pub use sync::GitSync;

/// Main GitOps engine for repository operations and change tracking
pub struct GitOpsEngine {
    config: GitConfig,
    repository: Option<Repository>,
}

impl std::fmt::Debug for GitOpsEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitOpsEngine")
            .field("config", &self.config)
            .field("repository", &self.repository.is_some())
            .finish()
    }
}

impl GitOpsEngine {
    /// Create a new GitOpsEngine with configuration.
    ///
    /// Validates the essential git config fields before constructing the engine.
    pub fn new(config: GitConfig) -> Result<Self> {
        // Validate git config fields
        let mut errors: Vec<String> = Vec::new();

        if config.repo_path.trim().is_empty() {
            errors.push("git.repo_path must not be empty".into());
        }
        if config.branch.trim().is_empty() {
            errors.push("git.branch must not be empty".into());
        }

        if !errors.is_empty() {
            return Err(ConfigError::Multiple(errors).into());
        }

        Ok(Self {
            config,
            repository: None,
        })
    }

    /// Initialize or open the state repository
    pub async fn initialize(&mut self) -> Result<()> {
        let repo_path = PathBuf::from(&self.config.repo_path);

        let repository = spawn_blocking(move || -> Result<Repository> {
            if repo_path.exists() {
                Repository::open(&repo_path)
                    .with_context(|| format!("Failed to open repository at {:?}", repo_path))
            } else {
                // Create parent directories if they don't exist
                if let Some(parent) = repo_path.parent() {
                    std::fs::create_dir_all(parent).with_context(|| {
                        format!("Failed to create parent directories for {:?}", parent)
                    })?;
                }

                let repo = Repository::init(&repo_path).with_context(|| {
                    format!("Failed to initialize repository at {:?}", repo_path)
                })?;

                // Set up initial configuration
                let mut config = repo.config()?;
                config.set_str("user.name", "FlowLink GitOps")?;
                config.set_str("user.email", "gitops@flowlink.local")?;
                config.set_bool("core.autocrlf", false)?;

                info!("Initialized new GitOps repository at {:?}", repo_path);
                Ok(repo)
            }
        })
        .await??;

        self.repository = Some(repository);
        Ok(())
    }

    /// Get reference to the repository, initializing if needed
    pub async fn repository(&mut self) -> Result<&Repository> {
        if self.repository.is_none() {
            self.initialize().await?;
        }
        Ok(self.repository.as_ref().unwrap())
    }

    /// Commit current state changes with metadata
    pub async fn commit_state_change(
        &mut self,
        message: &str,
        change_type: ChangeType,
        metadata: Option<Value>,
    ) -> Result<Oid> {
        let repo = self.repository().await?;

        spawn_blocking({
            let repo_path = repo.path().parent().unwrap().to_path_buf();
            let message = message.to_string();
            let _config = self.config.clone();

            move || -> Result<Oid> {
                let repo = Repository::open(&repo_path)?;

                // Stage all changes
                let mut index = repo.index()?;
                index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
                index.write()?;

                // Create tree from index
                let tree_id = index.write_tree()?;
                let tree = repo.find_tree(tree_id)?;

                // Get parent commit if it exists
                let parent_commit = match repo.head() {
                    Ok(head) => Some(head.peel_to_commit()?),
                    Err(_) => None,
                };

                // Create signature
                let signature = Signature::now("FlowLink GitOps", "gitops@flowlink.local")?;

                // Build commit message with metadata
                let full_message = if let Some(meta) = metadata {
                    format!(
                        "{}\n\nChange-Type: {:?}\nMetadata: {}",
                        message,
                        change_type,
                        serde_json::to_string(&meta)?
                    )
                } else {
                    format!("{}\n\nChange-Type: {:?}", message, change_type)
                };

                // Create commit
                let commit_id = match parent_commit {
                    Some(parent) => repo.commit(
                        Some("HEAD"),
                        &signature,
                        &signature,
                        &full_message,
                        &tree,
                        &[&parent],
                    )?,
                    None => repo.commit(
                        Some("HEAD"),
                        &signature,
                        &signature,
                        &full_message,
                        &tree,
                        &[],
                    )?,
                };

                info!(
                    "Created commit {} for change type {:?}",
                    commit_id, change_type
                );
                Ok(commit_id)
            }
        })
        .await?
    }

    /// Get the current repository status
    pub async fn get_status(&mut self) -> Result<RepoStatus> {
        let repo = self.repository().await?;

        spawn_blocking({
            let repo_path = repo.path().parent().unwrap().to_path_buf();

            move || -> Result<RepoStatus> {
                let repo = Repository::open(&repo_path)?;

                let mut status_options = StatusOptions::new();
                status_options.include_ignored(false);
                status_options.include_untracked(true);

                let statuses = repo.statuses(Some(&mut status_options))?;

                let mut modified_files = Vec::new();
                let mut untracked_files = Vec::new();
                let mut deleted_files = Vec::new();

                for entry in statuses.iter() {
                    if let Some(path) = entry.path() {
                        let file_path = PathBuf::from(path);
                        let status = entry.status();

                        if status.contains(git2::Status::WT_MODIFIED)
                            || status.contains(git2::Status::INDEX_MODIFIED)
                        {
                            modified_files.push(file_path.clone());
                        }

                        if status.contains(git2::Status::WT_NEW) {
                            untracked_files.push(file_path.clone());
                        }

                        if status.contains(git2::Status::WT_DELETED)
                            || status.contains(git2::Status::INDEX_DELETED)
                        {
                            deleted_files.push(file_path);
                        }
                    }
                }

                let head_commit = match repo.head() {
                    Ok(head) => Some(head.target().unwrap().to_string()),
                    Err(_) => None,
                };

                let is_clean = modified_files.is_empty()
                    && untracked_files.is_empty()
                    && deleted_files.is_empty();

                Ok(RepoStatus {
                    is_clean,
                    head_commit,
                    modified_files,
                    untracked_files,
                    deleted_files,
                    branch: get_current_branch(&repo)?.unwrap_or_else(|| "HEAD".to_string()),
                })
            }
        })
        .await?
    }

    /// Get commit history with optional limit
    pub async fn get_commit_history(&mut self, limit: Option<usize>) -> Result<Vec<CommitInfo>> {
        let repo = self.repository().await?;

        spawn_blocking({
            let repo_path = repo.path().parent().unwrap().to_path_buf();

            move || -> Result<Vec<CommitInfo>> {
                let repo = Repository::open(&repo_path)?;

                let mut revwalk = repo.revwalk()?;
                revwalk.push_head()?;
                revwalk.set_sorting(git2::Sort::TIME)?;

                let mut commits = Vec::new();
                let max_commits = limit.unwrap_or(100);

                for (count, oid) in revwalk.enumerate() {
                    if count >= max_commits {
                        break;
                    }

                    let oid = oid?;
                    let commit = repo.find_commit(oid)?;

                    let author_name = commit.author().name().unwrap_or("Unknown").to_string();
                    let author_email = commit
                        .author()
                        .email()
                        .unwrap_or("unknown@local")
                        .to_string();
                    let message = commit.message().unwrap_or("No message").to_string();
                    let timestamp = commit.time().seconds();

                    // Parse change type from commit message
                    let change_type = parse_change_type_from_message(&message);

                    commits.push(CommitInfo {
                        id: oid.to_string(),
                        author_name,
                        author_email,
                        timestamp,
                        message,
                        change_type,
                        parent_ids: commit.parent_ids().map(|id| id.to_string()).collect(),
                    });
                }

                Ok(commits)
            }
        })
        .await?
    }

    /// Create a backup snapshot of the current state
    pub async fn create_snapshot(&mut self, snapshot_type: BackupType) -> Result<String> {
        let repo = self.repository().await?;

        spawn_blocking({
            let repo_path = repo.path().parent().unwrap().to_path_buf();

            move || -> Result<String> {
                let repo = Repository::open(&repo_path)?;

                // Get current HEAD
                let head_commit = repo.head()?.peel_to_commit()?;
                let _commit_id = head_commit.id();

                // Create tag for snapshot
                let tag_name = format!(
                    "snapshot-{}-{}",
                    snapshot_type.to_string().to_lowercase(),
                    chrono::Utc::now().format("%Y%m%d-%H%M%S")
                );

                let signature = Signature::now("FlowLink GitOps", "gitops@flowlink.local")?;

                repo.tag(
                    &tag_name,
                    &head_commit.as_object(),
                    &signature,
                    &format!("Snapshot: {:?} at {}", snapshot_type, chrono::Utc::now()),
                    false,
                )?;

                info!("Created snapshot tag: {}", tag_name);
                Ok(tag_name)
            }
        })
        .await?
    }

    /// Validate repository integrity
    pub async fn validate_integrity(&mut self) -> Result<IntegrityStatus> {
        let repo = self.repository().await?;

        spawn_blocking({
            let repo_path = repo.path().parent().unwrap().to_path_buf();

            move || -> Result<IntegrityStatus> {
                let repo = Repository::open(&repo_path)?;

                let mut issues = Vec::new();
                let mut warnings = Vec::new();

                // Check if repository is corrupt
                match repo.odb() {
                    Ok(_odb) => {
                        // Basic ODB access test
                        debug!("Repository ODB accessible");
                    }
                    Err(e) => {
                        issues.push(format!("ODB corruption: {}", e));
                    }
                }

                // Verify HEAD is valid
                match repo.head() {
                    Ok(head) => match head.peel_to_commit() {
                        Ok(_) => debug!("HEAD points to valid commit"),
                        Err(e) => issues.push(format!("HEAD corruption: {}", e)),
                    },
                    Err(_) => warnings.push("No HEAD found (empty repository)".to_string()),
                }

                // Check for dangling objects (basic check)
                let mut revwalk = repo.revwalk()?;
                revwalk.push_head().ok(); // Don't fail if no HEAD

                let mut commit_count = 0;
                for oid in revwalk {
                    match oid {
                        Ok(oid) => match repo.find_commit(oid) {
                            Ok(_) => commit_count += 1,
                            Err(e) => issues.push(format!("Corrupted commit {}: {}", oid, e)),
                        },
                        Err(e) => issues.push(format!("Revwalk error: {}", e)),
                    }

                    // Limit check to avoid long delays
                    if commit_count > 1000 {
                        break;
                    }
                }

                let is_healthy = issues.is_empty();

                Ok(IntegrityStatus {
                    is_healthy,
                    issues,
                    warnings,
                    last_checked: chrono::Utc::now(),
                })
            }
        })
        .await?
    }
}

/// Helper function to get current branch name
fn get_current_branch(repo: &Repository) -> Result<Option<String>> {
    match repo.head() {
        Ok(head) => {
            if head.is_branch() {
                Ok(head.shorthand().map(|s| s.to_string()))
            } else {
                Ok(None) // Detached HEAD
            }
        }
        Err(_) => Ok(None), // No HEAD (empty repo)
    }
}

/// Parse change type from commit message
fn parse_change_type_from_message(message: &str) -> Option<ChangeType> {
    if let Some(start) = message.find("Change-Type: ") {
        let rest = &message[start + 13..];
        let type_str = rest.split('\n').next().unwrap_or(rest).trim();

        match type_str {
            "StateUpdate" => Some(ChangeType::StateUpdate),
            "ConfigChange" => Some(ChangeType::ConfigChange),
            "PolicyUpdate" => Some(ChangeType::PolicyUpdate),
            "Backup" => Some(ChangeType::Backup),
            "Rollback" => Some(ChangeType::Rollback),
            "DriftCorrection" => Some(ChangeType::DriftCorrection),
            _ => None,
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_config() -> GitConfig {
        let temp_dir = TempDir::new().unwrap();
        GitConfig {
            repo_path: temp_dir
                .path()
                .join("test-repo")
                .to_string_lossy()
                .to_string(),
            remote_url: None,
            branch: "main".to_string(),
            sync_strategy: crate::config::SyncStrategy::Realtime,
            signing_key: None,
        }
    }

    #[tokio::test]
    async fn test_initialize_new_repository() {
        let config = create_test_config();
        let mut engine = GitOpsEngine::new(config).unwrap();

        engine.initialize().await.unwrap();

        let repo = engine.repository().await.unwrap();
        assert!(repo.is_empty().unwrap_or(true)); // New repo should be empty
    }

    #[tokio::test]
    async fn test_get_status_empty_repo() {
        let config = create_test_config();
        let mut engine = GitOpsEngine::new(config).unwrap();

        engine.initialize().await.unwrap();
        let status = engine.get_status().await.unwrap();

        assert!(status.is_clean);
        assert!(status.head_commit.is_none());
        assert!(status.modified_files.is_empty());
    }

    #[tokio::test]
    async fn test_validate_integrity_new_repo() {
        let config = create_test_config();
        let mut engine = GitOpsEngine::new(config).unwrap();

        engine.initialize().await.unwrap();
        let integrity = engine.validate_integrity().await.unwrap();

        assert!(integrity.is_healthy);
        assert!(integrity.issues.is_empty());
    }

    #[tokio::test]
    async fn test_parse_change_type() {
        let message_with_type = "Update system state\n\nChange-Type: StateUpdate\nMetadata: {}";
        let change_type = parse_change_type_from_message(message_with_type);
        assert_eq!(change_type, Some(ChangeType::StateUpdate));

        let message_without_type = "Regular commit message";
        let change_type = parse_change_type_from_message(message_without_type);
        assert_eq!(change_type, None);
    }

    #[tokio::test]
    async fn test_create_snapshot() {
        let config = create_test_config();
        let mut engine = GitOpsEngine::new(config).unwrap();

        engine.initialize().await.unwrap();

        // Create an initial commit first
        std::fs::write(
            std::path::Path::new(&engine.config.repo_path).join("test.txt"),
            "test content",
        )
        .unwrap();

        let commit_id = engine
            .commit_state_change("Initial commit", ChangeType::StateUpdate, None)
            .await
            .unwrap();

        let snapshot_tag = engine
            .create_snapshot(BackupType::StateSnapshot)
            .await
            .unwrap();
        assert!(snapshot_tag.starts_with("snapshot-statesnapshot-"));
    }
}
