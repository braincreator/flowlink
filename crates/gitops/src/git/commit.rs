use anyhow::Result;
use git2::{Repository, Signature, Oid};

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
        
        let commit_id = self.repo.commit(
            Some("HEAD"),
            signature,
            signature,
            message,
            &tree,
            parents,
        )?;
        
        Ok(commit_id)
    }
}