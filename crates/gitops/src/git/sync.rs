//! Git sync operations — push, pull, remote management
//!
//! Uses git2 for actual git remote operations. Push and pull perform real
//! network operations when a remote URL is configured.

use anyhow::{Context, Result};
use git2::{FetchOptions, PushOptions, RemoteCallbacks, Repository};
use tracing::{debug, info, warn};

use crate::config::GitConfig;

/// Git sync engine — handles push/pull/remote operations
pub struct GitSync {
    config: GitConfig,
}

impl GitSync {
    pub fn new(config: GitConfig) -> Self {
        Self { config }
    }

    /// Open the git repository
    fn open_repo(&self) -> Result<Repository> {
        Repository::open(&self.config.repo_path)
            .with_context(|| format!("Failed to open repository at {:?}", self.config.repo_path))
    }

    /// Get the current branch name (resolving from config or HEAD)
    fn current_branch(&self, repo: &Repository) -> Result<String> {
        if !self.config.branch.is_empty() {
            return Ok(self.config.branch.clone());
        }
        // Resolve from HEAD
        let head = repo.head().context("Failed to resolve HEAD")?;
        let name = head
            .shorthand()
            .context("HEAD is detached — cannot determine branch")?;
        Ok(name.to_string())
    }

    /// Build remote callbacks with SSH/auth support
    fn remote_callbacks() -> RemoteCallbacks<'static> {
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(|_url, username_from_url, allowed_types| {
            // Try SSH agent first, then default credentials
            if allowed_types.contains(git2::CredentialType::SSH_KEY) {
                git2::Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
            } else if allowed_types.contains(git2::CredentialType::USER_PASS_PLAINTEXT) {
                // For HTTPS with token auth, read from env
                if let Ok(token) = std::env::var("FLOWLINK_GIT_TOKEN") {
                    git2::Cred::userpass_plaintext(username_from_url.unwrap_or("git"), &token)
                } else {
                    Err(git2::Error::from_str("No authentication credentials available"))
                }
            } else {
                Err(git2::Error::from_str("Unsupported credential type"))
            }
        });
        callbacks
    }

    /// Push local state to remote
    pub async fn push(&self) -> Result<()> {
        let remote_url = match &self.config.remote_url {
            Some(url) => url.clone(),
            None => {
                debug!("No remote URL configured, skipping push");
                return Ok(());
            }
        };

        let repo = self.open_repo()?;
        let branch = self.current_branch(&repo)?;
        let refspec = format!("refs/heads/{}:refs/heads/{}", branch, branch);

        info!("Pushing {} to remote {}", refspec, remote_url);

        // Ensure remote "origin" exists
        let mut remote = match repo.find_remote("origin") {
            Ok(r) => r,
            Err(_) => {
                info!("Remote 'origin' not found, creating from config");
                repo.remote("origin", &remote_url)
                    .context("Failed to create remote 'origin'")?;
                repo.find_remote("origin")?
            }
        };

        let mut callbacks = Self::remote_callbacks();
        callbacks.push_transfer_progress(|_current, _total, _bytes| {
            // Silent progress — can be enhanced with metrics
        });

        let mut push_opts = PushOptions::new();
        push_opts.remote_callbacks(callbacks);

        remote
            .push(&[&refspec], Some(&mut push_opts))
            .with_context(|| format!("Failed to push {} to {}", refspec, remote_url))?;

        info!("Push completed successfully");
        Ok(())
    }

    /// Pull remote state via fetch + fast-forward merge
    pub async fn pull(&self) -> Result<()> {
        let remote_url = match &self.config.remote_url {
            Some(url) => url.clone(),
            None => {
                debug!("No remote URL configured, skipping pull");
                return Ok(());
            }
        };

        let repo = self.open_repo()?;
        let branch = self.current_branch(&repo)?;
        let refspec = format!("refs/heads/{}:refs/remotes/origin/{}", branch, branch);

        info!("Pulling from remote {} on branch {}", remote_url, branch);

        // Ensure remote "origin" exists
        let mut remote = match repo.find_remote("origin") {
            Ok(r) => r,
            Err(_) => {
                info!("Remote 'origin' not found, creating from config");
                repo.remote("origin", &remote_url)
                    .context("Failed to create remote 'origin'")?;
                repo.find_remote("origin")?
            }
        };

        // Fetch from remote
        let mut callbacks = Self::remote_callbacks();
        let mut fetch_opts = FetchOptions::new();
        fetch_opts.remote_callbacks(callbacks);

        remote
            .fetch(&[&refspec], Some(&mut fetch_opts), None)
            .with_context(|| format!("Failed to fetch from {}", remote_url))?;

        // Fast-forward merge: set branch to fetched remote ref
        let remote_ref_name = format!("refs/remotes/origin/{}", branch);
        let remote_ref = repo
            .find_reference(&remote_ref_name)
            .with_context(|| format!("Remote branch {} not found after fetch", branch))?;
        let remote_oid = remote_ref.target().context("Remote ref has no target")?;

        // Check if we're already up to date
        let head_oid = repo.head()?.target().context("HEAD has no target")?;
        if head_oid == remote_oid {
            info!("Already up to date");
            return Ok(());
        }

        // Perform fast-forward
        let mut head_ref = repo.head()?.resolve().context("Failed to resolve HEAD reference")?;
        head_ref
            .set_target(remote_oid, "Fast-forward pull")
            .context("Failed to fast-forward — local changes may conflict")?;

        // Checkout the new HEAD to update working tree
        repo.checkout_head(None)
            .context("Failed to checkout after fast-forward")?;

        info!("Pull completed: fast-forwarded to {}", &remote_oid.to_string()[..8]);
        Ok(())
    }

    /// Initialize remote — add "origin" remote if it doesn't exist
    pub async fn init_remote(&self) -> Result<()> {
        let url = match &self.config.remote_url {
            Some(url) => url.clone(),
            None => {
                debug!("No remote URL configured, skipping remote init");
                return Ok(());
            }
        };

        info!("Initializing remote: {}", url);

        let repo = self.open_repo()?;

        match repo.find_remote("origin") {
            Ok(existing) => {
                // Verify URL matches
                if existing.url().unwrap_or("") != url {
                    warn!(
                        "Remote 'origin' already exists with URL {}, expected {}",
                        existing.url().unwrap_or("(none)"),
                        url
                    );
                } else {
                    info!("Remote 'origin' already configured with correct URL");
                }
            }
            Err(_) => {
                repo.remote("origin", &url)
                    .with_context(|| format!("Failed to add remote 'origin' with URL {}", url))?;
                info!("Added remote 'origin' with URL {}", url);
            }
        }

        Ok(())
    }

    /// Full sync cycle: pull → push
    pub async fn full_sync(&self) -> Result<()> {
        self.pull().await.context("Failed to pull")?;
        self.push().await.context("Failed to push")?;
        Ok(())
    }

    /// Get remote status (URL, connected, ahead/behind)
    pub async fn remote_status(&self) -> Result<RemoteStatus> {
        let repo = self.open_repo()?;

        let remote_info = repo.find_remote("origin").ok().and_then(|r| {
            r.url().map(|url| RemoteInfo {
                name: "origin".to_string(),
                url: url.to_string(),
            })
        });

        let (ahead, behind) = if remote_info.is_some() {
            let branch = self.current_branch(&repo)?;
            let ahead = self.count_ahead(&repo, &branch).unwrap_or(0);
            let behind = self.count_behind(&repo, &branch).unwrap_or(0);
            (ahead, behind)
        } else {
            (0, 0)
        };

        Ok(RemoteStatus {
            remote: remote_info,
            ahead,
            behind,
        })
    }

    /// Count commits ahead of remote
    fn count_ahead(&self, repo: &Repository, branch: &str) -> Result<usize> {
        let local_oid = repo.revparse_single(&format!("refs/heads/{}", branch))?.id();
        let remote_oid = repo.revparse_single(&format!("refs/remotes/origin/{}", branch))?.id();

        if local_oid == remote_oid {
            return Ok(0);
        }

        let merge_base = repo.merge_base(local_oid, remote_oid)?;
        let mut revwalk = repo.revwalk()?;
        revwalk.push(local_oid)?;
        revwalk.hide(merge_base)?;
        Ok(revwalk.count())
    }

    /// Count commits behind remote
    fn count_behind(&self, repo: &Repository, branch: &str) -> Result<usize> {
        let local_oid = repo.revparse_single(&format!("refs/heads/{}", branch))?.id();
        let remote_oid = repo.revparse_single(&format!("refs/remotes/origin/{}", branch))?.id();

        if local_oid == remote_oid {
            return Ok(0);
        }

        let merge_base = repo.merge_base(local_oid, remote_oid)?;
        let mut revwalk = repo.revwalk()?;
        revwalk.push(remote_oid)?;
        revwalk.hide(merge_base)?;
        Ok(revwalk.count())
    }
}

/// Information about a git remote
#[derive(Debug, Clone, serde::Serialize)]
pub struct RemoteInfo {
    pub name: String,
    pub url: String,
}

/// Status of remote synchronization
#[derive(Debug, Clone, serde::Serialize)]
pub struct RemoteStatus {
    pub remote: Option<RemoteInfo>,
    pub ahead: usize,
    pub behind: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SyncStrategy;
    use git2::{Repository, Signature};

    fn setup_local_repo() -> (GitSync, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        let sig = Signature::now("Test", "test@example.com").unwrap();
        let tree_oid = {
            let mut builder = repo.treebuilder(None).unwrap();
            builder
                .insert("test.txt", repo.blob(b"hello").unwrap(), 0o100644)
                .unwrap();
            builder.write().unwrap()
        };
        let tree = repo.find_tree(tree_oid).unwrap();
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "Initial commit",
            &tree,
            &[],
        )
        .unwrap();

        let config = GitConfig {
            repo_path: dir.path().to_string_lossy().to_string(),
            remote_url: None,
            branch: "main".to_string(),
            sync_strategy: SyncStrategy::Realtime,
            signing_key: None,
        };
        (GitSync::new(config), dir)
    }

    #[tokio::test]
    async fn test_push_no_remote_skips() {
        let (sync, _dir) = setup_local_repo();
        // Should not error — just skip
        sync.push().await.unwrap();
    }

    #[tokio::test]
    async fn test_pull_no_remote_skips() {
        let (sync, _dir) = setup_local_repo();
        sync.pull().await.unwrap();
    }

    #[tokio::test]
    async fn test_init_remote_no_url_skips() {
        let (sync, _dir) = setup_local_repo();
        sync.init_remote().await.unwrap();
    }

    #[tokio::test]
    async fn test_full_sync_no_remote() {
        let (sync, _dir) = setup_local_repo();
        sync.full_sync().await.unwrap();
    }

    #[tokio::test]
    async fn test_init_remote_adds_origin() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        let sig = Signature::now("Test", "test@example.com").unwrap();
        let tree_oid = {
            let mut builder = repo.treebuilder(None).unwrap();
            builder
                .insert("test.txt", repo.blob(b"hello").unwrap(), 0o100644)
                .unwrap();
            builder.write().unwrap()
        };
        let tree = repo.find_tree(tree_oid).unwrap();
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "Initial",
            &tree,
            &[],
        )
        .unwrap();

        let config = GitConfig {
            repo_path: dir.path().to_string_lossy().to_string(),
            remote_url: Some("https://github.com/example/repo.git".to_string()),
            branch: "main".to_string(),
            sync_strategy: SyncStrategy::Realtime,
            signing_key: None,
        };
        let sync = GitSync::new(config);

        sync.init_remote().await.unwrap();

        // Verify remote was added
        let remote = repo.find_remote("origin").unwrap();
        assert_eq!(remote.url().unwrap(), "https://github.com/example/repo.git");
    }

    #[tokio::test]
    async fn test_init_remote_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        let sig = Signature::now("Test", "test@example.com").unwrap();
        let tree_oid = {
            let mut builder = repo.treebuilder(None).unwrap();
            builder
                .insert("test.txt", repo.blob(b"hello").unwrap(), 0o100644)
                .unwrap();
            builder.write().unwrap()
        };
        let tree = repo.find_tree(tree_oid).unwrap();
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "Initial",
            &tree,
            &[],
        )
        .unwrap();

        let config = GitConfig {
            repo_path: dir.path().to_string_lossy().to_string(),
            remote_url: Some("https://github.com/example/repo.git".to_string()),
            branch: "main".to_string(),
            sync_strategy: SyncStrategy::Realtime,
            signing_key: None,
        };
        let sync = GitSync::new(config);

        // Call twice — should not error
        sync.init_remote().await.unwrap();
        sync.init_remote().await.unwrap();
    }

    #[tokio::test]
    async fn test_remote_status_no_remote() {
        let (sync, _dir) = setup_local_repo();
        let status = sync.remote_status().await.unwrap();
        assert!(status.remote.is_none());
        assert_eq!(status.ahead, 0);
        assert_eq!(status.behind, 0);
    }
}
