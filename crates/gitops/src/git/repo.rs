use anyhow::Result;
use git2::Repository;
use std::path::Path;

pub struct RepositoryManager {
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