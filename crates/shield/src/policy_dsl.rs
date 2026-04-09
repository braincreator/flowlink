// FlowLink Shield — YAML-based Policy DSL
// Configurable security rules with priorities, conditions, and time-based access.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PolicyAction {
    #[serde(rename = "allow")]
    Allow,
    #[serde(rename = "deny")]
    Deny,
    #[serde(rename = "ask")]
    Ask,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Condition {
    CommandPattern { pattern: String },
    CommandRegex { regex: String },
    UserIn { users: Vec<String> },
    UserNotIn { users: Vec<String> },
    GroupIn { groups: Vec<String> },
    UidEq { uid: u32 },
    UidGt { uid: u32 },
    PathUnder { path: String },
    PathNotUnder { path: String },
    OriginIs { origin: String },
    ThreatLevelMin { level: String },
    TimeBetween { start: String, end: String },
    DayOfWeek { days: Vec<String> },
    RiskScoreGt { score: u8 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub action: PolicyAction,
    pub conditions: Vec<Condition>,
    pub priority: i32,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool { true }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicySet {
    pub version: String,
    pub default_action: PolicyAction,
    #[serde(default)]
    pub rules: Vec<PolicyRule>,
}

#[derive(Debug, Clone)]
pub struct EvalContext {
    pub user: String,
    pub groups: Vec<String>,
    pub uid: u32,
    pub origin: String,
    pub cwd: String,
    pub threat_level: String,
    pub risk_score: u8,
    pub now: chrono::DateTime<chrono::Utc>,
}

impl Default for EvalContext {
    fn default() -> Self {
        Self {
            user: String::new(),
            groups: Vec::new(),
            uid: 0,
            origin: String::new(),
            cwd: String::new(),
            threat_level: String::new(),
            risk_score: 0,
            now: chrono::Utc::now(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PolicyDecision {
    pub action: PolicyAction,
    pub matched_rule: Option<String>,
    pub reason: String,
}

pub struct PolicyEngine {
    policies: PolicySet,
}

impl PolicyEngine {
    pub fn load_from_yaml(yaml: &str) -> anyhow::Result<Self> {
        let policies: PolicySet = serde_yaml::from_str(yaml)?;
        Ok(Self { policies })
    }

    pub fn load_from_file(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::load_from_yaml(&content)
    }

    pub fn policies(&self) -> &PolicySet {
        &self.policies
    }

    pub fn evaluate(&self, command: &str, ctx: &EvalContext) -> PolicyDecision {
        let mut rules: Vec<&PolicyRule> = self.policies.rules.iter()
            .filter(|r| r.enabled)
            .collect();
        rules.sort_by(|a, b| b.priority.cmp(&a.priority));

        for rule in rules {
            if rule.conditions.iter().all(|c| self.matches(c, command, ctx)) {
                return PolicyDecision {
                    action: rule.action.clone(),
                    matched_rule: Some(rule.name.clone()),
                    reason: rule.description.clone().unwrap_or_else(|| format!("matched rule: {}", rule.name)),
                };
            }
        }

        PolicyDecision {
            action: self.policies.default_action.clone(),
            matched_rule: None,
            reason: "no rule matched, using default action".into(),
        }
    }

    fn matches(&self, cond: &Condition, command: &str, ctx: &EvalContext) -> bool {
        match cond {
            Condition::CommandPattern { pattern } => {
                let pat = glob_pattern_to_regex(pattern);
                if let Ok(re) = regex::Regex::new(&pat) {
                    re.is_match(command)
                } else { false }
            }
            Condition::CommandRegex { regex } => {
                if let Ok(re) = regex::Regex::new(regex) {
                    re.is_match(command)
                } else { false }
            }
            Condition::UserIn { users } => users.iter().any(|u| u == &ctx.user),
            Condition::UserNotIn { users } => !users.iter().any(|u| u == &ctx.user),
            Condition::GroupIn { groups } => groups.iter().any(|g| ctx.groups.contains(g)),
            Condition::UidEq { uid } => ctx.uid == *uid,
            Condition::UidGt { uid } => ctx.uid >= *uid,
            Condition::PathUnder { path } => ctx.cwd.starts_with(path) || ctx.cwd == *path,
            Condition::PathNotUnder { path } => !ctx.cwd.starts_with(path) && ctx.cwd != *path,
            Condition::OriginIs { origin } => ctx.origin == *origin,
            Condition::ThreatLevelMin { level } => {
                let order = |l: &str| match l.to_uppercase().as_str() {
                    "L3" | "CRITICAL" => 3,
                    "L2" | "HIGH" => 2,
                    "L1" | "MEDIUM" => 1,
                    _ => 0,
                };
                order(&ctx.threat_level) >= order(level)
            }
            Condition::TimeBetween { start, end } => {
                let now = ctx.now.time();
                let s = parse_time(start);
                let e = parse_time(end);
                s.map(|s| e.map(|e| now >= s && now <= e).unwrap_or(false)).unwrap_or(false)
            }
            Condition::DayOfWeek { days } => {
                let dow = ctx.now.format("%a").to_string().to_lowercase();
                days.iter().any(|d| d.to_lowercase() == dow)
            }
            Condition::RiskScoreGt { score } => ctx.risk_score > *score,
        }
    }
}

fn glob_pattern_to_regex(pattern: &str) -> String {
    let mut re = String::from("(?i)^");
    for c in pattern.chars() {
        match c {
            '*' => re.push_str(".*"),
            '?' => re.push('.'),
            '.' | '+' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' | '\\' => {
                re.push('\\');
                re.push(c);
            }
            _ => re.push(c),
        }
    }
    re.push('$');
    re
}

fn parse_time(s: &str) -> Option<chrono::NaiveTime> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 { return None; }
    let h = parts[0].parse::<u32>().ok()?;
    let m = parts[1].parse::<u32>().ok()?;
    chrono::NaiveTime::from_hms_opt(h, m, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASIC_YAML: &str = r#"
version: "1.0"
default_action: ask
rules:
  - name: "allow-ls"
    action: allow
    priority: 100
    enabled: true
    conditions:
      - !CommandPattern
          pattern: "ls *"
  - name: "block-rm"
    action: deny
    priority: 90
    enabled: true
    conditions:
      - !CommandRegex
          regex: "rm -rf /"
"#;

    #[test]
    fn parse_basic_yaml() {
        let engine = PolicyEngine::load_from_yaml(BASIC_YAML).unwrap();
        assert_eq!(engine.policies().rules.len(), 2);
        assert_eq!(engine.policies().version, "1.0");
    }

    #[test]
    fn allow_match() {
        let engine = PolicyEngine::load_from_yaml(BASIC_YAML).unwrap();
        let ctx = EvalContext::default();
        let dec = engine.evaluate("ls -la", &ctx);
        assert_eq!(dec.action, PolicyAction::Allow);
        assert_eq!(dec.matched_rule.as_deref(), Some("allow-ls"));
    }

    #[test]
    fn deny_match() {
        let engine = PolicyEngine::load_from_yaml(BASIC_YAML).unwrap();
        let ctx = EvalContext::default();
        let dec = engine.evaluate("rm -rf /", &ctx);
        assert_eq!(dec.action, PolicyAction::Deny);
    }

    #[test]
    fn default_action_fallback() {
        let engine = PolicyEngine::load_from_yaml(BASIC_YAML).unwrap();
        let ctx = EvalContext::default();
        let dec = engine.evaluate("some random command", &ctx);
        assert_eq!(dec.action, PolicyAction::Ask);
        assert!(dec.matched_rule.is_none());
    }

    #[test]
    fn priority_ordering() {
        let yaml = r#"
version: "1.0"
default_action: deny
rules:
  - name: "low-priority-allow"
    action: allow
    priority: 10
    enabled: true
    conditions:
      - !CommandPattern
          pattern: "test *"
  - name: "high-priority-deny"
    action: deny
    priority: 100
    enabled: true
    conditions:
      - !CommandPattern
          pattern: "test *"
"#;
        let engine = PolicyEngine::load_from_yaml(yaml).unwrap();
        let dec = engine.evaluate("test command", &EvalContext::default());
        assert_eq!(dec.action, PolicyAction::Deny);
        assert_eq!(dec.matched_rule.as_deref(), Some("high-priority-deny"));
    }

    #[test]
    fn disabled_rules_skipped() {
        let yaml = r#"
version: "1.0"
default_action: deny
rules:
  - name: "disabled-rule"
    action: allow
    priority: 100
    enabled: false
    conditions:
      - !CommandPattern
          pattern: "test *"
"#;
        let engine = PolicyEngine::load_from_yaml(yaml).unwrap();
        let dec = engine.evaluate("test command", &EvalContext::default());
        assert_eq!(dec.action, PolicyAction::Deny);
    }

    #[test]
    fn user_condition() {
        let yaml = r#"
version: "1.0"
default_action: deny
rules:
  - name: "admin-allow"
    action: allow
    priority: 100
    enabled: true
    conditions:
      - !UserIn
          users: ["admin", "root"]
"#;
        let engine = PolicyEngine::load_from_yaml(yaml).unwrap();
        let mut ctx = EvalContext::default();
        ctx.user = "admin".into();
        assert_eq!(engine.evaluate("anything", &ctx).action, PolicyAction::Allow);

        ctx.user = "guest".into();
        assert_eq!(engine.evaluate("anything", &ctx).action, PolicyAction::Deny);
    }

    #[test]
    fn user_not_in_condition() {
        let yaml = r#"
version: "1.0"
default_action: allow
rules:
  - name: "block-guest"
    action: deny
    priority: 100
    enabled: true
    conditions:
      - !UserNotIn
          users: ["admin", "root"]
"#;
        let engine = PolicyEngine::load_from_yaml(yaml).unwrap();
        let mut ctx = EvalContext::default();
        ctx.user = "guest".into();
        assert_eq!(engine.evaluate("cmd", &ctx).action, PolicyAction::Deny);

        ctx.user = "admin".into();
        assert_eq!(engine.evaluate("cmd", &ctx).action, PolicyAction::Allow);
    }

    #[test]
    fn uid_conditions() {
        let yaml = r#"
version: "1.0"
default_action: allow
rules:
  - name: "root-deny"
    action: deny
    priority: 100
    enabled: true
    conditions:
      - !UidEq { uid: 0 }
  - name: "high-uid-warn"
    action: ask
    priority: 50
    enabled: true
    conditions:
      - !UidGt { uid: 1000 }
"#;
        let engine = PolicyEngine::load_from_yaml(yaml).unwrap();
        let mut ctx = EvalContext::default();
        ctx.uid = 0;
        assert_eq!(engine.evaluate("cmd", &ctx).action, PolicyAction::Deny);

        ctx.uid = 1001;
        assert_eq!(engine.evaluate("cmd", &ctx).action, PolicyAction::Ask);

        ctx.uid = 500;
        assert_eq!(engine.evaluate("cmd", &ctx).action, PolicyAction::Allow);
    }

    #[test]
    fn path_conditions() {
        let yaml = r#"
version: "1.0"
default_action: allow
rules:
  - name: "protect-prod"
    action: deny
    priority: 100
    enabled: true
    conditions:
      - !PathUnder
          path: "/opt/prod"
"#;
        let engine = PolicyEngine::load_from_yaml(yaml).unwrap();
        let mut ctx = EvalContext::default();
        ctx.cwd = "/opt/prod/app".into();
        assert_eq!(engine.evaluate("cmd", &ctx).action, PolicyAction::Deny);

        ctx.cwd = "/opt/dev/app".into();
        assert_eq!(engine.evaluate("cmd", &ctx).action, PolicyAction::Allow);
    }

    #[test]
    fn multi_condition_all_match() {
        let yaml = r#"
version: "1.0"
default_action: allow
rules:
  - name: "root-guest-deny"
    action: deny
    priority: 100
    enabled: true
    conditions:
      - !UidEq { uid: 0 }
      - !UserIn
          users: ["guest"]
"#;
        let engine = PolicyEngine::load_from_yaml(yaml).unwrap();
        let mut ctx = EvalContext::default();
        ctx.uid = 0;
        ctx.user = "guest".into();
        assert_eq!(engine.evaluate("cmd", &ctx).action, PolicyAction::Deny);

        ctx.user = "root".into();
        assert_eq!(engine.evaluate("cmd", &ctx).action, PolicyAction::Allow);
    }

    #[test]
    fn day_of_week() {
        let yaml = r#"
version: "1.0"
default_action: deny
rules:
  - name: "weekdays-only"
    action: allow
    priority: 100
    enabled: true
    conditions:
      - !DayOfWeek
          days: ["mon", "tue", "wed", "thu", "fri"]
"#;
        let engine = PolicyEngine::load_from_yaml(yaml).unwrap();
        let mut ctx = EvalContext::default();
        // Monday 2026-04-06
        ctx.now = chrono::DateTime::parse_from_rfc3339("2026-04-06T12:00:00Z").unwrap().with_timezone(&chrono::Utc);
        assert_eq!(engine.evaluate("cmd", &ctx).action, PolicyAction::Allow);

        // Sunday 2026-04-12
        ctx.now = chrono::DateTime::parse_from_rfc3339("2026-04-12T12:00:00Z").unwrap().with_timezone(&chrono::Utc);
        assert_eq!(engine.evaluate("cmd", &ctx).action, PolicyAction::Deny);
    }

    #[test]
    fn time_between() {
        let yaml = r#"
version: "1.0"
default_action: deny
rules:
  - name: "business-hours"
    action: allow
    priority: 100
    enabled: true
    conditions:
      - !TimeBetween
          start: "09:00"
          end: "18:00"
"#;
        let engine = PolicyEngine::load_from_yaml(yaml).unwrap();
        let mut ctx = EvalContext::default();
        ctx.now = chrono::DateTime::parse_from_rfc3339("2026-04-06T10:00:00Z").unwrap().with_timezone(&chrono::Utc);
        assert_eq!(engine.evaluate("cmd", &ctx).action, PolicyAction::Allow);

        ctx.now = chrono::DateTime::parse_from_rfc3339("2026-04-06T20:00:00Z").unwrap().with_timezone(&chrono::Utc);
        assert_eq!(engine.evaluate("cmd", &ctx).action, PolicyAction::Deny);
    }

    #[test]
    fn risk_score_gt() {
        let yaml = r#"
version: "1.0"
default_action: allow
rules:
  - name: "high-risk-deny"
    action: deny
    priority: 100
    enabled: true
    conditions:
      - !RiskScoreGt { score: 80 }
"#;
        let engine = PolicyEngine::load_from_yaml(yaml).unwrap();
        let mut ctx = EvalContext::default();
        ctx.risk_score = 90;
        assert_eq!(engine.evaluate("cmd", &ctx).action, PolicyAction::Deny);

        ctx.risk_score = 50;
        assert_eq!(engine.evaluate("cmd", &ctx).action, PolicyAction::Allow);
    }

    #[test]
    fn threat_level_min() {
        let yaml = r#"
version: "1.0"
default_action: allow
rules:
  - name: "critical-block"
    action: deny
    priority: 100
    enabled: true
    conditions:
      - !ThreatLevelMin
          level: "L3"
"#;
        let engine = PolicyEngine::load_from_yaml(yaml).unwrap();
        let mut ctx = EvalContext::default();
        ctx.threat_level = "L3".into();
        assert_eq!(engine.evaluate("cmd", &ctx).action, PolicyAction::Deny);

        ctx.threat_level = "L1".into();
        assert_eq!(engine.evaluate("cmd", &ctx).action, PolicyAction::Allow);
    }

    #[test]
    fn origin_condition() {
        let yaml = r#"
version: "1.0"
default_action: allow
rules:
  - name: "block-container"
    action: deny
    priority: 100
    enabled: true
    conditions:
      - !OriginIs
          origin: "container"
"#;
        let engine = PolicyEngine::load_from_yaml(yaml).unwrap();
        let mut ctx = EvalContext::default();
        ctx.origin = "container".into();
        assert_eq!(engine.evaluate("cmd", &ctx).action, PolicyAction::Deny);

        ctx.origin = "ssh".into();
        assert_eq!(engine.evaluate("cmd", &ctx).action, PolicyAction::Allow);
    }

    #[test]
    fn empty_ruleset() {
        let yaml = r#"
version: "1.0"
default_action: allow
rules: []
"#;
        let engine = PolicyEngine::load_from_yaml(yaml).unwrap();
        let dec = engine.evaluate("anything", &EvalContext::default());
        assert_eq!(dec.action, PolicyAction::Allow);
    }

    #[test]
    fn invalid_yaml() {
        let result = PolicyEngine::load_from_yaml("not: valid: yaml: [");
        assert!(result.is_err());
    }

    #[test]
    fn group_condition() {
        let yaml = r#"
version: "1.0"
default_action: deny
rules:
  - name: "ops-allow"
    action: allow
    priority: 100
    enabled: true
    conditions:
      - !GroupIn
          groups: ["ops", "wheel"]
"#;
        let engine = PolicyEngine::load_from_yaml(yaml).unwrap();
        let mut ctx = EvalContext::default();
        ctx.groups = vec!["ops".into()];
        assert_eq!(engine.evaluate("cmd", &ctx).action, PolicyAction::Allow);

        ctx.groups = vec!["users".into()];
        assert_eq!(engine.evaluate("cmd", &ctx).action, PolicyAction::Deny);
    }

    #[test]
    fn glob_pattern_wildcard() {
        let yaml = r#"
version: "1.0"
default_action: deny
rules:
  - name: "cat-files"
    action: allow
    priority: 100
    enabled: true
    conditions:
      - !CommandPattern
          pattern: "cat *"
"#;
        let engine = PolicyEngine::load_from_yaml(yaml).unwrap();
        assert_eq!(engine.evaluate("cat /etc/passwd", &EvalContext::default()).action, PolicyAction::Allow);
        assert_eq!(engine.evaluate("cat", &EvalContext::default()).action, PolicyAction::Deny);
    }

    #[test]
    fn description_in_decision() {
        let yaml = r#"
version: "1.0"
default_action: deny
rules:
  - name: "test"
    description: "Test rule description"
    action: allow
    priority: 100
    enabled: true
    conditions:
      - !UserIn
          users: ["admin"]
"#;
        let engine = PolicyEngine::load_from_yaml(yaml).unwrap();
        let mut ctx = EvalContext::default();
        ctx.user = "admin".into();
        let dec = engine.evaluate("cmd", &ctx);
        assert_eq!(dec.reason, "Test rule description");
    }

    #[test]
    fn load_from_file() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), BASIC_YAML).unwrap();
        let engine = PolicyEngine::load_from_file(tmp.path()).unwrap();
        assert_eq!(engine.policies().rules.len(), 2);
    }
}
