use anyhow::Result;
use git2::{Oid, Repository, Signature};

pub struct CommitManager {
    repo: Repository,
}

impl CommitManager {
    pub fn new(repo: Repository) -> Self {
        Self { repo }
    }

    pub fn create_commit(
        &self,
        message: &str,
        signature: &Signature,
        tree_id: Oid,
        parents: &[&git2::Commit],
    ) -> Result<Oid> {
        let tree = self.repo.find_tree(tree_id)?;

        let commit_id =
            self.repo
                .commit(Some("HEAD"), signature, signature, message, &tree, parents)?;

        Ok(commit_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn init_test_repo() -> (TempDir, Repository) {
        let tmp = TempDir::new().unwrap();
        let repo = Repository::init(tmp.path()).unwrap();
        // Configure user so commits work
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@test.com").unwrap();
        (tmp, repo)
    }

    fn make_signature() -> git2::Signature<'static> {
        git2::Signature::new("Test User", "test@test.com", &git2::Time::new(0, 0)).unwrap()
    }

    fn create_initial_commit(repo: &Repository) -> Oid {
        let sig = make_signature();
        let tree_id = {
            let mut index = repo.index().unwrap();
            // Create an empty tree (no files)
            let tree_id = index.write_tree().unwrap();
            tree_id
        };
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "initial",
            &repo.find_tree(tree_id).unwrap(),
            &[],
        )
        .unwrap()
    }

    #[test]
    fn test_create_commit_initial() {
        let (_tmp, repo) = init_test_repo();
        let mgr = CommitManager::new(Repository::open(_tmp.path()).unwrap());
        let sig = make_signature();

        // Create an empty tree
        let tree_id = {
            let mut index = mgr.repo.index().unwrap();
            index.write_tree().unwrap()
        };
        let tree = mgr.repo.find_tree(tree_id).unwrap();

        let oid = mgr
            .create_commit("initial commit", &sig, tree_id, &[])
            .unwrap();
        assert!(!oid.is_zero());

        // Verify the commit was created
        let commit = mgr.repo.find_commit(oid).unwrap();
        assert_eq!(commit.message().unwrap(), "initial commit");
    }

    #[test]
    fn test_create_commit_with_parent() {
        let (tmp, repo) = init_test_repo();
        let initial_oid = create_initial_commit(&repo);

        // Use the same repo handle to avoid cross-handle commit issues
        let mgr = CommitManager::new(repo);
        let sig = make_signature();

        let tree_id = {
            let mut index = mgr.repo.index().unwrap();
            index.write_tree().unwrap()
        };

        // Look up the parent commit from the same repo handle
        let initial_commit = mgr.repo.find_commit(initial_oid).unwrap();
        let oid = mgr
            .create_commit("second commit", &sig, tree_id, &[&initial_commit])
            .unwrap();
        let commit = mgr.repo.find_commit(oid).unwrap();
        assert_eq!(commit.message().unwrap(), "second commit");
        assert_eq!(commit.parent_count(), 1);
        assert_eq!(commit.parent_id(0).unwrap(), initial_oid);
    }

    #[test]
    fn test_create_commit_with_metadata_in_message() {
        let (_tmp, repo) = init_test_repo();
        let mgr = CommitManager::new(Repository::open(_tmp.path()).unwrap());
        let sig = make_signature();

        let tree_id = {
            let mut index = mgr.repo.index().unwrap();
            index.write_tree().unwrap()
        };

        let message = format!(
            "Update nginx config\n\nChange-Type: ConfigChange\nMetadata: {}",
            serde_json::json!({"reason": "performance tuning"})
        );

        let oid = mgr.create_commit(&message, &sig, tree_id, &[]).unwrap();
        let commit = mgr.repo.find_commit(oid).unwrap();
        let msg = commit.message().unwrap();
        assert!(msg.contains("Change-Type: ConfigChange"));
        assert!(msg.contains("performance tuning"));
    }

    #[test]
    fn test_create_commit_multiple_parents() {
        let (tmp, repo) = init_test_repo();
        let first_oid = create_initial_commit(&repo);

        // Create a second branch
        let sig = make_signature();
        let tree_id = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        let second_oid = repo
            .commit(
                Some("refs/heads/feature"),
                &sig,
                &sig,
                "feature commit",
                &repo.find_tree(tree_id).unwrap(),
                &[],
            )
            .unwrap();

        // Use the same repo handle to avoid cross-handle commit issues
        let mgr = CommitManager::new(repo);

        // Look up parent commits from the same repo handle
        let first_commit = mgr.repo.find_commit(first_oid).unwrap();
        let second_commit = mgr.repo.find_commit(second_oid).unwrap();

        let merge_oid = mgr
            .create_commit(
                "merge commit",
                &sig,
                tree_id,
                &[&first_commit, &second_commit],
            )
            .unwrap();

        let merge_commit = mgr.repo.find_commit(merge_oid).unwrap();
        assert_eq!(merge_commit.parent_count(), 2);
    }

    #[test]
    fn test_commit_author_info_preserved() {
        let (_tmp, repo) = init_test_repo();
        let mgr = CommitManager::new(Repository::open(_tmp.path()).unwrap());
        let sig = make_signature();

        let tree_id = {
            let mut index = mgr.repo.index().unwrap();
            index.write_tree().unwrap()
        };

        let oid = mgr
            .create_commit("test author", &sig, tree_id, &[])
            .unwrap();
        let commit = repo.find_commit(oid).unwrap();
        assert_eq!(commit.author().name().unwrap(), "Test User");
        assert_eq!(commit.author().email().unwrap(), "test@test.com");
    }
}
