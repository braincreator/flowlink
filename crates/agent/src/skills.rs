// Skills — named capability packages pushed to agents
// Port of internal/agent/skills.go

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use base64::{engine::general_purpose::STANDARD, Engine as _};

/// A skill with files, metadata, and instructions.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Skill {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    pub description: String,
    pub instructions: String,
    #[serde(default)]
    pub files: Vec<SkillFile>,
    #[serde(default)]
    pub tools_allowed: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_model: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

/// A file belonging to a skill.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SkillFile {
    pub path: String,
    pub content: String, // base64 encoded
    pub executable: bool,
}

/// Manages skill storage on disk.
pub struct SkillManager {
    skills_dir: PathBuf,
}

impl SkillManager {
    /// Create a new skill manager. Creates the skills directory if needed.
    pub fn new(base_dir: &str) -> Result<Self> {
        let dir = PathBuf::from(base_dir).join("skills");
        fs::create_dir_all(&dir)
            .with_context(|| format!("Failed to create skills dir: {}", dir.display()))?;
        Ok(Self { skills_dir: dir })
    }

    /// Install (save) a skill to disk. Writes skill JSON + files.
    pub fn install(&self, skill: &mut Skill) -> Result<()> {
        if skill.id.is_empty() {
            anyhow::bail!("skill ID cannot be empty");
        }
        if skill.instructions.is_empty() {
            anyhow::bail!("skill instructions cannot be empty");
        }

        let now = chrono::Utc::now().timestamp();
        if skill.created_at == 0 {
            skill.created_at = now;
        }
        skill.updated_at = now;

        // Write skill files to skills_dir/name/
        let skill_dir = self.skill_path(&skill.id);
        fs::create_dir_all(&skill_dir)?;

        for file in &skill.files {
            let file_path = skill_dir.join(&file.path);
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let decoded = STANDARD
                .decode(&file.content)
                .with_context(|| format!("Failed to decode file {}", file.path))?;
            fs::write(&file_path, &decoded)?;
            if file.executable {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let perms = fs::metadata(&file_path)?.permissions();
                    fs::set_permissions(
                        &file_path,
                        fs::Permissions::from_mode(perms.mode() | 0o111),
                    )?;
                }
            }
        }

        // Write skill metadata as JSON
        let meta_path = skill_dir.join("skill.json");
        let data = serde_json::to_string_pretty(skill)?;
        fs::write(&meta_path, data)?;

        info!(
            "Installed skill: {} ({} files)",
            skill.name,
            skill.files.len()
        );
        Ok(())
    }

    /// List all installed skills.
    pub fn list(&self) -> Result<Vec<Skill>> {
        let mut skills = Vec::new();
        if !self.skills_dir.exists() {
            return Ok(skills);
        }

        for entry in fs::read_dir(&self.skills_dir)? {
            let entry = entry?;
            let meta_path = entry.path().join("skill.json");
            if !meta_path.exists() {
                continue;
            }
            match fs::read_to_string(&meta_path) {
                Ok(data) => match serde_json::from_str::<Skill>(&data) {
                    Ok(skill) => skills.push(skill),
                    Err(e) => warn!(
                        "Skipping corrupted skill in {}: {e}",
                        entry.path().display()
                    ),
                },
                Err(e) => warn!("Cannot read skill in {}: {e}", entry.path().display()),
            }
        }

        skills.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(skills)
    }

    /// Delete a skill by name/id.
    pub fn delete(&self, name: &str) -> Result<()> {
        let dir = self.skill_path(name);
        if dir.exists() {
            fs::remove_dir_all(&dir)
                .with_context(|| format!("Failed to delete skill: {}", name))?;
            info!("Deleted skill: {name}");
        }
        Ok(())
    }

    /// Get a single skill by name/id.
    pub fn get(&self, name: &str) -> Result<Skill> {
        let meta_path = self.skill_path(name).join("skill.json");
        let data =
            fs::read_to_string(&meta_path).with_context(|| format!("Skill not found: {name}"))?;
        let skill: Skill = serde_json::from_str(&data)?;
        Ok(skill)
    }

    /// Return the path to a skill's directory.
    pub fn skill_path(&self, name: &str) -> PathBuf {
        let safe = sanitize_id(name);
        self.skills_dir.join(safe)
    }

    /// Compute SHA256 hash (first 16 bytes, hex) of a skill.
    pub fn hash(&self, name: &str) -> Result<String> {
        let skill = self.get(name)?;
        let data = serde_json::to_vec(&skill)?;
        let digest = Sha256::digest(&data);
        Ok(hex::encode(&digest[..16]))
    }

    /// Search skills by keyword (case-insensitive match on name, description, id).
    pub fn search(&self, query: &str) -> Result<Vec<Skill>> {
        let query = query.to_lowercase();
        let all = self.list()?;
        Ok(all
            .into_iter()
            .filter(|s| {
                s.name.to_lowercase().contains(&query)
                    || s.description.to_lowercase().contains(&query)
                    || s.id.to_lowercase().contains(&query)
            })
            .collect())
    }
}

fn sanitize_id(id: &str) -> String {
    id.replace('/', "_").replace('\\', "_").replace("..", "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_install_and_get() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = SkillManager::new(dir.path().to_str().unwrap()).unwrap();

        let mut skill = Skill {
            id: "test-1".into(),
            name: "Test Skill".into(),
            version: "1.0.0".into(),
            description: "A test skill".into(),
            instructions: "Do something".into(),
            files: vec![SkillFile {
                path: "run.sh".into(),
                content: STANDARD.encode(b"#!/bin/bash\necho hi"),
                executable: true,
            }],
            tools_allowed: vec!["exec".into()],
            llm_provider: None,
            llm_model: None,
            created_at: 0,
            updated_at: 0,
            metadata: HashMap::new(),
        };

        mgr.install(&mut skill).unwrap();
        let loaded = mgr.get("test-1").unwrap();
        assert_eq!(loaded.name, "Test Skill");
        assert_ne!(loaded.created_at, 0);

        // File should exist
        assert!(mgr.skill_path("test-1").join("run.sh").exists());
    }

    #[test]
    fn test_list_and_delete() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = SkillManager::new(dir.path().to_str().unwrap()).unwrap();

        for i in 0..3 {
            let mut s = Skill {
                id: format!("skill-{i}"),
                name: format!("Skill {i}"),
                version: String::new(),
                description: String::new(),
                instructions: "test".into(),
                files: vec![],
                tools_allowed: vec![],
                llm_provider: None,
                llm_model: None,
                created_at: 0,
                updated_at: 0,
                metadata: HashMap::new(),
            };
            mgr.install(&mut s).unwrap();
        }

        assert_eq!(mgr.list().unwrap().len(), 3);
        mgr.delete("skill-1").unwrap();
        assert_eq!(mgr.list().unwrap().len(), 2);
        // Delete non-existent should not error
        mgr.delete("skill-1").unwrap();
    }

    #[test]
    fn test_validation() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = SkillManager::new(dir.path().to_str().unwrap()).unwrap();

        let mut s = Skill {
            id: String::new(),
            name: "Empty ID".into(),
            version: String::new(),
            description: String::new(),
            instructions: "test".into(),
            files: vec![],
            tools_allowed: vec![],
            llm_provider: None,
            llm_model: None,
            created_at: 0,
            updated_at: 0,
            metadata: HashMap::new(),
        };
        assert!(mgr.install(&mut s).is_err());

        s.id = "has-id".into();
        s.instructions = String::new();
        assert!(mgr.install(&mut s).is_err());
    }

    #[test]
    fn test_search() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = SkillManager::new(dir.path().to_str().unwrap()).unwrap();

        for (id, name, desc) in [
            ("py-1", "Python Runner", "Run Python scripts"),
            ("py-2", "Python Linter", "Lint Python code"),
            ("sh-1", "Bash Executor", "Execute bash"),
        ] {
            let mut s = Skill {
                id: id.into(),
                name: name.into(),
                version: String::new(),
                description: desc.into(),
                instructions: "test".into(),
                files: vec![],
                tools_allowed: vec![],
                llm_provider: None,
                llm_model: None,
                created_at: 0,
                updated_at: 0,
                metadata: HashMap::new(),
            };
            mgr.install(&mut s).unwrap();
        }

        assert_eq!(mgr.search("Python").unwrap().len(), 2);
        assert_eq!(mgr.search("bash").unwrap().len(), 1);
        assert_eq!(mgr.search("golang").unwrap().len(), 0);
    }
}
