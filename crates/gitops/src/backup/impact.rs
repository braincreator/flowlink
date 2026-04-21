//! Impact analysis for backup operations
//!
//! Analyzes commands and arguments to determine what resources are at risk
//! and what type of backup should be performed.

use crate::types::*;
use std::path::PathBuf;
use tracing::{debug, warn};

/// Result of impact analysis
#[derive(Debug, Clone)]
pub struct ImpactAssessment {
    /// Type of backup recommended
    pub backup_type: BackupType,
    /// Paths that would be affected
    pub affected_paths: Vec<PathBuf>,
    /// Risk level of the operation
    pub risk_level: ImpactLevel,
    /// Human-readable description
    pub description: String,
}

/// Impact level for backup operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ImpactLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Analyzes commands to determine impact and backup requirements
pub struct ImpactAnalyzer {
    /// SQL keywords that indicate destructive operations
    destructive_sql_keywords: Vec<&'static str>,
}

impl ImpactAnalyzer {
    /// Create a new impact analyzer
    pub fn new() -> Self {
        Self {
            destructive_sql_keywords: vec![
                "DROP TABLE",
                "DROP DATABASE",
                "TRUNCATE",
                "DELETE FROM",
                "ALTER TABLE",
            ],
        }
    }

    /// Analyze a command and its arguments to determine impact
    pub fn analyze(&self, command: &str, args: &[String]) -> ImpactAssessment {
        debug!("Analyzing command: {} with {} args", command, args.len());

        let command_lower = command.to_lowercase();
        let basename = Self::get_basename(&command_lower);

        match basename.as_str() {
            "rm" | "rmdir" | "shred" => self.analyze_file_deletion(command, args),
            "docker" => self.analyze_docker_command(args),
            "psql" | "mysql" | "sqlite3" | "mongosh" => self.analyze_db_command(command, args),
            "systemctl" => self.analyze_systemctl_command(args),
            "apt" | "apt-get" | "yum" | "dnf" | "apk" | "pacman" => {
                self.analyze_package_manager(args)
            }
            "git" => self.analyze_git_command(args),
            "chmod" | "chown" => self.analyze_permission_change(command, args),
            _ => self.default_assessment(command, args),
        }
    }

    /// Get basename of command (handle paths)
    fn get_basename(command: &str) -> String {
        command.rsplit('/').next().unwrap_or(command).to_string()
    }

    /// Analyze file deletion commands (rm, rmdir, shred)
    fn analyze_file_deletion(&self, _command: &str, args: &[String]) -> ImpactAssessment {
        let mut affected_paths = Vec::new();
        let mut risk_level = ImpactLevel::Medium;
        let mut description = String::from("File deletion operation");

        for arg in args {
            // Skip flags
            if arg.starts_with('-') {
                // Check for recursive force flags
                if arg.contains('r') && arg.contains('f') {
                    risk_level = ImpactLevel::High;
                    description = String::from("Recursive forced file deletion");
                }
                continue;
            }

            // Convert to PathBuf
            let path = PathBuf::from(arg);
            affected_paths.push(path);
        }

        // Check for critical paths
        if affected_paths.iter().any(|p| {
            let path_str = p.to_string_lossy();
            path_str.starts_with("/etc/")
                || path_str.starts_with("/usr/")
                || path_str.starts_with("/var/")
                || path_str.starts_with("/home/")
        }) {
            risk_level = risk_level.max(ImpactLevel::High);
        }

        ImpactAssessment {
            backup_type: BackupType::FileSnapshot {
                paths: affected_paths
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect(),
                include_hashes: true,
            },
            affected_paths,
            risk_level,
            description,
        }
    }

    /// Analyze Docker commands
    fn analyze_docker_command(&self, args: &[String]) -> ImpactAssessment {
        if args.is_empty() {
            return self.default_assessment("docker", args);
        }

        let subcommand = args[0].to_lowercase();
        let mut containers = Vec::new();
        let mut risk_level = ImpactLevel::Medium;
        let mut description = String::from("Docker operation");

        match subcommand.as_str() {
            "rm" | "stop" | "kill" | "pause" => {
                // Collect container names/IDs
                for arg in args.iter().skip(1) {
                    if !arg.starts_with('-') {
                        containers.push(arg.clone());
                    }
                }
                risk_level = ImpactLevel::High;
                description = format!("Docker container {} operation", subcommand);
            }
            "rmi" => {
                // Image removal
                risk_level = ImpactLevel::High;
                description = String::from("Docker image removal");
            }
            "compose" | "container" => {
                // Check for destructive sub-subcommands
                if args.len() > 1 {
                    let sub_sub = args[1].to_lowercase();
                    if matches!(sub_sub.as_str(), "down" | "rm" | "stop") {
                        risk_level = ImpactLevel::High;
                        description = format!("Docker compose {} operation", sub_sub);
                    }
                }
            }
            _ => {
                // Other docker commands are lower risk
                risk_level = ImpactLevel::Low;
            }
        }

        ImpactAssessment {
            backup_type: BackupType::DockerState {
                containers: containers.clone(),
                include_volumes: true,
                include_env: true,
            },
            affected_paths: Vec::new(),
            risk_level,
            description,
        }
    }

    /// Analyze database commands
    fn analyze_db_command(&self, command: &str, args: &[String]) -> ImpactAssessment {
        let mut databases = Vec::new();
        let mut tables = Vec::new();
        let mut risk_level = ImpactLevel::Medium;
        let mut description = String::from("Database operation");

        // Check for SQL in arguments
        for arg in args {
            let arg_upper = arg.to_uppercase();

            // Check for destructive SQL keywords
            for keyword in &self.destructive_sql_keywords {
                if arg_upper.contains(keyword) {
                    risk_level = ImpactLevel::Critical;
                    description = format!("Destructive database operation: {}", keyword);

                    // Try to extract table/database names
                    if keyword.starts_with("DROP TABLE") || keyword.starts_with("ALTER TABLE") {
                        if let Some(name) =
                            Self::extract_identifier_after_keyword(&arg_upper, keyword)
                        {
                            tables.push(name);
                        }
                    } else if keyword.starts_with("DROP DATABASE") {
                        if let Some(name) =
                            Self::extract_identifier_after_keyword(&arg_upper, keyword)
                        {
                            databases.push(name);
                        }
                    }
                }
            }

            // Check for -c flag with SQL
            if arg_upper.starts_with("-C") || arg_upper == "-C" {
                // Next arg might contain SQL
                continue;
            }
        }

        let db_type = Self::detect_db_type(command);

        ImpactAssessment {
            backup_type: BackupType::DatabaseDump {
                db_type,
                databases,
                tables: if tables.is_empty() {
                    None
                } else {
                    Some(tables)
                },
                format: DumpFormat::Sql,
            },
            affected_paths: Vec::new(),
            risk_level,
            description,
        }
    }

    /// Detect database type from command
    fn detect_db_type(command: &str) -> DbType {
        let cmd_lower = command.to_lowercase();
        if cmd_lower.contains("psql") {
            DbType::PostgreSQL
        } else if cmd_lower.contains("mysql") {
            DbType::MySQL
        } else if cmd_lower.contains("sqlite") {
            DbType::SQLite
        } else if cmd_lower.contains("mongo") {
            DbType::MongoDB
        } else {
            DbType::PostgreSQL // Default
        }
    }

    /// Extract identifier after SQL keyword
    fn extract_identifier_after_keyword(sql: &str, keyword: &str) -> Option<String> {
        let after_keyword = sql.split(keyword).nth(1)?;
        let identifier = after_keyword
            .split_whitespace()
            .next()?
            .trim_matches(|c| c == '`' || c == '"' || c == '\'' || c == ';')
            .to_string();
        Some(identifier)
    }

    /// Analyze systemctl commands
    fn analyze_systemctl_command(&self, args: &[String]) -> ImpactAssessment {
        if args.is_empty() {
            return self.default_assessment("systemctl", args);
        }

        let subcommand = args[0].to_lowercase();
        let mut components = Vec::new();
        let mut description = String::from("System service operation");

        let risk_level = match subcommand.as_str() {
            "stop" | "disable" | "mask" | "reset-failed" => {
                for arg in args.iter().skip(1) {
                    if !arg.starts_with('-') && !arg.ends_with(".service") {
                        components.push(format!("{}.service", arg));
                    } else if !arg.starts_with('-') {
                        components.push(arg.clone());
                    }
                }
                description = format!("System service {} operation", subcommand);
                ImpactLevel::High
            }
            "restart" | "reload" => {
                for arg in args.iter().skip(1) {
                    if !arg.starts_with('-') {
                        components.push(arg.clone());
                    }
                }
                ImpactLevel::Medium
            }
            _ => ImpactLevel::Low,
        };

        ImpactAssessment {
            backup_type: BackupType::SystemConfig { components },
            affected_paths: Vec::new(),
            risk_level,
            description,
        }
    }

    /// Analyze package manager commands
    fn analyze_package_manager(&self, args: &[String]) -> ImpactAssessment {
        if args.is_empty() {
            return self.default_assessment("package-manager", args);
        }

        let subcommand = args[0].to_lowercase();
        let mut components = Vec::new();
        let mut description = String::from("Package management operation");

        let risk_level = match subcommand.as_str() {
            "remove" | "purge" | "autoremove" | "erase" => {
                for arg in args.iter().skip(1) {
                    if !arg.starts_with('-') {
                        components.push(format!("package:{}", arg));
                    }
                }
                description = String::from("Package removal");
                ImpactLevel::High
            }
            "install" | "upgrade" | "update" | "dist-upgrade" => {
                description = String::from("Package installation/upgrade");
                ImpactLevel::Medium
            }
            _ => ImpactLevel::Low,
        };

        ImpactAssessment {
            backup_type: BackupType::SystemConfig { components },
            affected_paths: Vec::new(),
            risk_level,
            description,
        }
    }

    /// Analyze git commands
    fn analyze_git_command(&self, args: &[String]) -> ImpactAssessment {
        if args.is_empty() {
            return self.default_assessment("git", args);
        }

        let subcommand = args[0].to_lowercase();
        let affected_paths = vec![PathBuf::from(".git")];
        let mut description = String::from("Git operation");

        let risk_level = match subcommand.as_str() {
            "reset" | "rebase" | "checkout" => {
                description = format!("Git {} operation", subcommand);
                ImpactLevel::High
            }
            "clean" => {
                description = String::from("Git clean (removes untracked files)");
                ImpactLevel::High
            }
            _ => ImpactLevel::Low,
        };

        ImpactAssessment {
            backup_type: BackupType::FileSnapshot {
                paths: affected_paths
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect(),
                include_hashes: true,
            },
            affected_paths,
            risk_level,
            description,
        }
    }

    /// Analyze permission change commands
    fn analyze_permission_change(&self, _command: &str, args: &[String]) -> ImpactAssessment {
        let mut affected_paths = Vec::new();
        let mut risk_level = ImpactLevel::Medium;
        let mut description = String::from("Permission change");

        for arg in args.iter().skip(1) {
            if arg.starts_with('-') {
                continue;
            }

            let path = PathBuf::from(arg);
            let path_str = path.to_string_lossy();

            // Check for critical security files
            if path_str.contains("/etc/shadow")
                || path_str.contains("/etc/passwd")
                || path_str.contains("/etc/ssh")
                || path_str.contains("/etc/ssl")
            {
                risk_level = ImpactLevel::Critical;
                description = String::from("Security-critical permission change");
            }

            affected_paths.push(path);
        }

        ImpactAssessment {
            backup_type: BackupType::FileSnapshot {
                paths: affected_paths
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect(),
                include_hashes: true,
            },
            affected_paths,
            risk_level,
            description,
        }
    }

    /// Default assessment for unknown commands
    fn default_assessment(&self, command: &str, args: &[String]) -> ImpactAssessment {
        warn!(
            "Unknown command '{}' with {} args, using default assessment",
            command,
            args.len()
        );

        ImpactAssessment {
            backup_type: BackupType::StateSnapshot,
            affected_paths: Vec::new(),
            risk_level: ImpactLevel::Low,
            description: format!("Unknown command: {}", command),
        }
    }
}

impl Default for ImpactAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_rm_command() {
        let analyzer = ImpactAnalyzer::new();
        let assessment = analyzer.analyze("rm", &["-rf".to_string(), "/tmp/test".to_string()]);

        assert!(matches!(
            assessment.backup_type,
            BackupType::FileSnapshot { .. }
        ));
        assert!(assessment
            .affected_paths
            .contains(&PathBuf::from("/tmp/test")));
        assert_eq!(assessment.risk_level, ImpactLevel::High);
    }

    #[test]
    fn test_analyze_docker_rm() {
        let analyzer = ImpactAnalyzer::new();
        let assessment = analyzer.analyze(
            "docker",
            &[
                "rm".to_string(),
                "container1".to_string(),
                "container2".to_string(),
            ],
        );

        if let BackupType::DockerState { containers, .. } = assessment.backup_type {
            assert!(containers.contains(&"container1".to_string()));
            assert!(containers.contains(&"container2".to_string()));
        } else {
            panic!("Expected DockerState backup type");
        }
        assert_eq!(assessment.risk_level, ImpactLevel::High);
    }

    #[test]
    fn test_analyze_sql_drop_table() {
        let analyzer = ImpactAnalyzer::new();
        let assessment = analyzer.analyze(
            "psql",
            &["-c".to_string(), "DROP TABLE users CASCADE".to_string()],
        );

        assert!(matches!(
            assessment.backup_type,
            BackupType::DatabaseDump { .. }
        ));
        assert_eq!(assessment.risk_level, ImpactLevel::Critical);
    }

    #[test]
    fn test_analyze_systemctl_stop() {
        let analyzer = ImpactAnalyzer::new();
        let assessment = analyzer.analyze("systemctl", &["stop".to_string(), "nginx".to_string()]);

        if let BackupType::SystemConfig { components } = assessment.backup_type {
            assert!(components.iter().any(|c| c.contains("nginx")));
        } else {
            panic!("Expected SystemConfig backup type");
        }
        assert_eq!(assessment.risk_level, ImpactLevel::High);
    }

    #[test]
    fn test_analyze_package_removal() {
        let analyzer = ImpactAnalyzer::new();
        let assessment = analyzer.analyze("apt", &["remove".to_string(), "nginx".to_string()]);

        assert!(matches!(
            assessment.backup_type,
            BackupType::SystemConfig { .. }
        ));
        assert_eq!(assessment.risk_level, ImpactLevel::High);
    }

    #[test]
    fn test_permission_change_critical() {
        let analyzer = ImpactAnalyzer::new();
        let assessment = analyzer.analyze("chmod", &["777".to_string(), "/etc/shadow".to_string()]);

        assert_eq!(assessment.risk_level, ImpactLevel::Critical);
        assert!(assessment.description.contains("Security-critical"));
    }

    #[test]
    fn test_basename_extraction() {
        assert_eq!(ImpactAnalyzer::get_basename("/usr/bin/rm"), "rm");
        assert_eq!(ImpactAnalyzer::get_basename("rm"), "rm");
        assert_eq!(
            ImpactAnalyzer::get_basename("/usr/local/bin/docker"),
            "docker"
        );
    }
}
