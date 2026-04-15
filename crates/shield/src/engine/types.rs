// FlowLink Shield — Core types for the analysis engine

use serde::Serialize;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ThreatLevel {
    Critical,
    High,
    Medium,
    #[allow(dead_code)]
    Low,
}

#[derive(Debug, Clone, Serialize)]
pub struct Threat {
    pub id: String,
    pub name: String,
    pub description: String,
    pub level: ThreatLevel,
    pub snapshot: bool,
    pub timeout_secs: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

impl Threat {
    pub(crate) fn critical(id: &str, name: &str, desc: String) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: desc,
            level: ThreatLevel::Critical,
            snapshot: true,
            timeout_secs: 60,
            suggestion: None,
        }
    }
    pub(crate) fn high(id: &str, name: &str, desc: String) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: desc,
            level: ThreatLevel::High,
            snapshot: false,
            timeout_secs: 60,
            suggestion: None,
        }
    }
    pub(crate) fn warn(id: &str, name: &str, desc: String) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: desc,
            level: ThreatLevel::Medium,
            snapshot: false,
            timeout_secs: 0,
            suggestion: None,
        }
    }

    pub(crate) fn with_suggestion(mut self, s: &str) -> Self {
        self.suggestion = Some(s.into());
        self
    }
}

impl fmt::Display for ThreatLevel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Critical => write!(f, "🚫 BLOCK"),
            Self::High => write!(f, "⛔ BLOCK"),
            Self::Medium => write!(f, "⚠️ WARN"),
            Self::Low => write!(f, "📝 LOG"),
        }
    }
}

pub struct Command {
    pub binary: String,
    pub args: Vec<String>,
    pub raw: String,
}

pub struct AnalysisResult {
    pub threat: Option<Threat>,
    pub level_used: u8,
    pub safe: bool,
}

#[allow(dead_code)]
pub struct PolicyAwareResult {
    pub allowed: bool,
    pub threat: Option<Threat>,
    pub policy_decision: Option<crate::policy_dsl::PolicyDecision>,
}

/// Extract basename from a path (e.g. "/usr/bin/rm" → "rm")
pub fn bn(p: &str) -> &str {
    p.rsplit('/').next().unwrap_or(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ═══════════════════════════════════════════
    // ThreatLevel
    // ═══════════════════════════════════════════

    #[test]
    fn threat_level_equality() {
        assert_eq!(ThreatLevel::Critical, ThreatLevel::Critical);
        assert_eq!(ThreatLevel::High, ThreatLevel::High);
        assert_ne!(ThreatLevel::Critical, ThreatLevel::Low);
    }

    #[test]
    fn threat_level_clone() {
        let level = ThreatLevel::Critical;
        let cloned = level.clone();
        assert_eq!(level, cloned);
    }

    #[test]
    fn threat_level_debug() {
        assert_eq!(format!("{:?}", ThreatLevel::Critical), "Critical");
        assert_eq!(format!("{:?}", ThreatLevel::High), "High");
        assert_eq!(format!("{:?}", ThreatLevel::Medium), "Medium");
        assert_eq!(format!("{:?}", ThreatLevel::Low), "Low");
    }

    #[test]
    fn threat_level_display() {
        assert_eq!(format!("{}", ThreatLevel::Critical), "🚫 BLOCK");
        assert_eq!(format!("{}", ThreatLevel::High), "⛔ BLOCK");
        assert_eq!(format!("{}", ThreatLevel::Medium), "⚠️ WARN");
        assert_eq!(format!("{}", ThreatLevel::Low), "📝 LOG");
    }

    #[test]
    fn threat_level_serialization() {
        let json = serde_json::to_string(&ThreatLevel::Critical).unwrap();
        assert!(json.contains("Critical"));
        let json = serde_json::to_string(&ThreatLevel::Low).unwrap();
        assert!(json.contains("Low"));
    }

    #[test]
    fn threat_level_deserialization_via_value() {
        let json = serde_json::to_string(&ThreatLevel::Critical).unwrap();
        let val: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(val, "Critical");
        let json = serde_json::to_string(&ThreatLevel::Low).unwrap();
        let val: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(val, "Low");
    }

    #[test]
    fn threat_level_roundtrip_via_value() {
        for level in &[
            ThreatLevel::Critical,
            ThreatLevel::High,
            ThreatLevel::Medium,
            ThreatLevel::Low,
        ] {
            let json = serde_json::to_string(level).unwrap();
            let val: serde_json::Value = serde_json::from_str(&json).unwrap();
            let expected = serde_json::to_value(level).unwrap();
            assert_eq!(val, expected);
        }
    }

    // ═══════════════════════════════════════════
    // Threat
    // ═══════════════════════════════════════════

    #[test]
    fn threat_critical_construction() {
        let t = Threat::critical("T001", "rm_rf", "rm -rf / detected".into());
        assert_eq!(t.id, "T001");
        assert_eq!(t.name, "rm_rf");
        assert_eq!(t.level, ThreatLevel::Critical);
        assert!(t.snapshot);
        assert_eq!(t.timeout_secs, 60);
    }

    #[test]
    fn threat_high_construction() {
        let t = Threat::high("T002", "docker_rm", "docker rm -f".into());
        assert_eq!(t.level, ThreatLevel::High);
        assert!(!t.snapshot);
        assert_eq!(t.timeout_secs, 60);
    }

    #[test]
    fn threat_warn_construction() {
        let t = Threat::warn("T003", "chmod", "chmod 777".into());
        assert_eq!(t.level, ThreatLevel::Medium);
        assert!(!t.snapshot);
        assert_eq!(t.timeout_secs, 0);
    }

    #[test]
    fn threat_debug() {
        let t = Threat::critical("T001", "rm_rf", "dangerous".into());
        let debug = format!("{:?}", t);
        assert!(debug.contains("T001"));
        assert!(debug.contains("rm_rf"));
    }

    #[test]
    fn threat_clone() {
        let t = Threat::high("T002", "test", "desc".into());
        let cloned = t.clone();
        assert_eq!(cloned.id, t.id);
        assert_eq!(cloned.level, t.level);
    }

    #[test]
    fn threat_serialization() {
        let t = Threat::critical("T001", "rm_rf", "rm -rf /".into());
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains("T001"));
        assert!(json.contains("rm_rf"));
        assert!(json.contains("Critical"));
    }

    #[test]
    fn threat_deserialization_via_value() {
        let t = Threat::critical("T001", "rm_rf", "rm -rf /".into());
        let json = serde_json::to_string(&t).unwrap();
        let val: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(val["id"], "T001");
        assert_eq!(val["level"], "Critical");
        assert_eq!(val["snapshot"], true);
    }

    #[test]
    fn threat_roundtrip_via_value() {
        let t = Threat::high("T005", "docker", "docker rm -f".into());
        let json = serde_json::to_string(&t).unwrap();
        let val: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(val["id"], "T005");
        assert_eq!(val["name"], "docker");
        assert_eq!(val["level"], "High");
    }

    // ═══════════════════════════════════════════
    // Command
    // ═══════════════════════════════════════════

    #[test]
    fn command_construction() {
        let cmd = Command {
            binary: "/usr/bin/rm".into(),
            args: vec!["-rf".into(), "/".into()],
            raw: "/usr/bin/rm -rf /".into(),
        };
        assert_eq!(cmd.binary, "/usr/bin/rm");
        assert_eq!(cmd.args.len(), 2);
        assert_eq!(cmd.raw, "/usr/bin/rm -rf /");
    }

    #[test]
    fn command_empty() {
        let cmd = Command {
            binary: String::new(),
            args: vec![],
            raw: String::new(),
        };
        assert!(cmd.binary.is_empty());
        assert!(cmd.args.is_empty());
        assert!(cmd.raw.is_empty());
    }

    // ═══════════════════════════════════════════
    // AnalysisResult
    // ═══════════════════════════════════════════

    #[test]
    fn analysis_result_safe() {
        let result = AnalysisResult {
            threat: None,
            level_used: 0,
            safe: true,
        };
        assert!(result.safe);
        assert!(result.threat.is_none());
    }

    #[test]
    fn analysis_result_threat() {
        let result = AnalysisResult {
            threat: Some(Threat::critical("T001", "rm", "desc".into())),
            level_used: 1,
            safe: false,
        };
        assert!(!result.safe);
        assert_eq!(result.level_used, 1);
        assert!(result.threat.is_some());
    }

    // ═══════════════════════════════════════════
    // bn() helper
    // ═══════════════════════════════════════════

    #[test]
    fn bn_full_path() {
        assert_eq!(bn("/usr/bin/rm"), "rm");
    }

    #[test]
    fn bn_basename() {
        assert_eq!(bn("rm"), "rm");
    }

    #[test]
    fn bn_nested_path() {
        assert_eq!(bn("/usr/local/bin/custom-tool"), "custom-tool");
    }

    #[test]
    fn bn_empty_string() {
        assert_eq!(bn(""), "");
    }

    #[test]
    fn bn_trailing_slash() {
        assert_eq!(bn("/usr/bin/"), "");
    }
}
