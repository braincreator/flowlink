// FlowLink Shield — Core types for the analysis engine

use serde::Serialize;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ThreatLevel { Critical, High, Medium, #[allow(dead_code)] Low }

#[derive(Debug, Clone, Serialize)]
pub struct Threat {
    pub id: String, pub name: String, pub description: String,
    pub level: ThreatLevel, pub snapshot: bool, pub timeout_secs: u64,
}

impl Threat {
    pub(crate) fn critical(id: &str, name: &str, desc: String) -> Self {
        Self { id: id.into(), name: name.into(), description: desc,
               level: ThreatLevel::Critical, snapshot: true, timeout_secs: 60 }
    }
    pub(crate) fn high(id: &str, name: &str, desc: String) -> Self {
        Self { id: id.into(), name: name.into(), description: desc,
               level: ThreatLevel::High, snapshot: false, timeout_secs: 60 }
    }
    pub(crate) fn warn(id: &str, name: &str, desc: String) -> Self {
        Self { id: id.into(), name: name.into(), description: desc,
               level: ThreatLevel::Medium, snapshot: false, timeout_secs: 0 }
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

pub struct Command { pub binary: String, pub args: Vec<String>, pub raw: String }

pub struct AnalysisResult { pub threat: Option<Threat>, pub level_used: u8, pub safe: bool }

#[allow(dead_code)]
pub struct PolicyAwareResult {
    pub allowed: bool,
    pub threat: Option<Threat>,
    pub policy_decision: Option<crate::policy_dsl::PolicyDecision>,
}

/// Extract basename from a path (e.g. "/usr/bin/rm" → "rm")
pub fn bn(p: &str) -> &str { p.rsplit('/').next().unwrap_or(p) }
