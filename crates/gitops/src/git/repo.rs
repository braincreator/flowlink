use anyhow::Result;
use git2::Repository;
use std::path::Path;

pub struct RepositoryManager {
    #[allow(dead_code)]
    repo: Repository,
}

impl RepositoryManager {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let repo = Repository::open(path)?;
        Ok(Self { repo })
    }

    pub fn init<P: AsRef<Path>>(path: P) -> Result<Self> {
        let repo = Repository::init(path)?;
        Ok(Self { repo })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_init_creates_new_repository() {
        let tmp = TempDir::new().unwrap();
        let repo_path = tmp.path().join("new-repo");

        let mgr = RepositoryManager::init(&repo_path).unwrap();
        assert!(repo_path.exists());

        // Verify it's a valid git repo
        let _repo = Repository::open(&repo_path).unwrap();
        // Verify the internal repo workdir points to the right location
        // Use canonicalize to handle macOS /var -> /private/var symlinks
        assert_eq!(
            mgr.repo.workdir().unwrap().canonicalize().unwrap(),
            repo_path.canonicalize().unwrap()
        );
    }

    #[test]
    fn test_init_creates_dot_git_directory() {
        let tmp = TempDir::new().unwrap();
        let repo_path = tmp.path().join("init-test");

        RepositoryManager::init(&repo_path).unwrap();
        assert!(repo_path.join(".git").exists());
    }

    #[test]
    fn test_new_opens_existing_repository() {
        let tmp = TempDir::new().unwrap();
        let repo_path = tmp.path().join("existing-repo");

        // Create the repo first
        Repository::init(&repo_path).unwrap();

        // Open it via RepositoryManager
        let mgr = RepositoryManager::new(&repo_path).unwrap();
        // A freshly initialized repo has no commits, so it's empty
        assert!(mgr.repo.is_empty().unwrap());
    }

    #[test]
    fn test_new_fails_on_nonexistent_path() {
        let result = RepositoryManager::new("/tmp/flowlink_test_nonexistent_repo_12345");
        assert!(result.is_err(), "opening nonexistent repo should fail");
    }

    #[test]
    fn test_new_fails_on_non_git_directory() {
        let tmp = TempDir::new().unwrap();
        let non_git_path = tmp.path().join("not-a-repo");
        std::fs::create_dir_all(&non_git_path).unwrap();

        let result = RepositoryManager::new(&non_git_path);
        assert!(result.is_err(), "opening a non-git directory should fail");
    }

    #[test]
    fn test_init_nested_directory() {
        let tmp = TempDir::new().unwrap();
        let nested = tmp.path().join("a").join("b").join("c").join("repo");

        let mgr = RepositoryManager::init(&nested).unwrap();
        assert!(nested.join(".git").exists());

        // Verify we can reopen it
        let mgr2 = RepositoryManager::new(&nested).unwrap();
        // Use canonicalize to handle macOS /var -> /private/var symlinks
        assert_eq!(
            mgr2.repo.workdir().unwrap().canonicalize().unwrap(),
            nested.canonicalize().unwrap()
        );
    }

    #[test]
    fn test_init_repo_is_empty() {
        let tmp = TempDir::new().unwrap();
        let repo_path = tmp.path().join("empty-repo");

        let mgr = RepositoryManager::init(&repo_path).unwrap();
        assert!(mgr.repo.is_empty().unwrap());
    }

    #[test]
    fn test_init_and_reopen_idempotent() {
        let tmp = TempDir::new().unwrap();
        let repo_path = tmp.path().join("idempotent-repo");

        // Init
        let mgr1 = RepositoryManager::init(&repo_path).unwrap();
        // Open
        let mgr2 = RepositoryManager::new(&repo_path).unwrap();

        // Both should reference the same repo path
        assert_eq!(mgr1.repo.path(), mgr2.repo.path());
    }
}
