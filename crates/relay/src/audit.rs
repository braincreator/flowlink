// FlowLink Relay — Audit Store
// In-memory store with JSONL persistence, query, export, and SIEM support

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::time::Duration;

use chrono::Utc;
use dashmap::DashMap;
use serde::Serialize;

use flowlink_core::channels::{AuditEvent, AuditEventType};

// ═══════════════════════════════════════════════
// Audit Store
// ═══════════════════════════════════════════════

pub struct AuditStore {
    events: DashMap<String, AuditEvent>,
    journal_path: PathBuf,
}

impl AuditStore {
    pub fn new(journal_path: &std::path::Path) -> Self {
        let store = Self {
            events: DashMap::new(),
            journal_path: journal_path.to_path_buf(),
        };
        // Load existing journal
        if let Err(e) = store.load_journal() {
            log::warn!("Failed to load audit journal: {}", e);
        }
        store
    }

    pub fn record(&self, event: AuditEvent) -> anyhow::Result<()> {
        let id = event.id.clone();
        let journal_path = self.journal_path.clone();
        let event_clone = event.clone();
        // Persist to journal (fire-and-forget, non-blocking)
        std::thread::spawn(move || {
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&journal_path) {
                if let Ok(line) = serde_json::to_string(&event_clone) {
                    let _ = writeln!(f, "{}", line);
                }
            }
        });
        self.events.insert(id, event);
        Ok(())
    }

    /// Blocking write — use in tests or shutdown hooks.
    pub fn record_sync(&self, event: AuditEvent) -> anyhow::Result<()> {
        let id = event.id.clone();
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&self.journal_path) {
            if let Ok(line) = serde_json::to_string(&event) {
                let _ = writeln!(f, "{}", line);
            }
        }
        self.events.insert(id, event);
        Ok(())
    }

    pub fn query(&self, filter: &AuditFilter) -> Vec<AuditEvent> {
        let _now_nanos = Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64;
        let mut results: Vec<AuditEvent> = self.events
            .iter()
            .filter(|e| {
                let ev = e.value();
                if let Some(ref agent_id) = filter.agent_id {
                    if &ev.agent_id != agent_id { return false; }
                }
                if let Some(ref event_type) = filter.event_type {
                    if ev.event_type.as_str() != event_type { return false; }
                }
                if let Some(since) = filter.since {
                    if ev.timestamp_nanos < since { return false; }
                }
                if let Some(until) = filter.until {
                    if ev.timestamp_nanos > until { return false; }
                }
                if let Some(min_risk) = filter.min_risk_score {
                    if ev.event_type.risk_score().is_none_or(|r| r < min_risk) { return false; }
                }
                true
            })
            .map(|e| e.value().clone())
            .collect();

        // Sort by timestamp descending
        results.sort_by(|a, b| b.timestamp_nanos.cmp(&a.timestamp_nanos));

        if let Some(limit) = filter.limit {
            results.truncate(limit);
        }

        results
    }

    pub fn export_json(&self, filter: &AuditFilter) -> String {
        let events = self.query(filter);
        serde_json::to_string_pretty(&events).unwrap_or_else(|_| "[]".into())
    }

    pub fn export_siem(&self, format: &SiemFormat, filter: &AuditFilter) -> String {
        let events = self.query(filter);
        match format {
            SiemFormat::Cef => events.iter().map(|e| self.to_cef(e)).collect::<Vec<_>>().join("\n"),
            SiemFormat::Leef => events.iter().map(|e| self.to_leef(e)).collect::<Vec<_>>().join("\n"),
            SiemFormat::Json => serde_json::to_string(&events).unwrap_or_else(|_| "[]".into()),
        }
    }

    pub fn stats(&self) -> AuditStats {
        let now_nanos = Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64;
        let one_day = 24 * 60 * 60 * 1_000_000_000u64;
        let cutoff = now_nanos.saturating_sub(one_day);

        let mut interceptions_24h = 0usize;
        let mut denied_24h = 0usize;
        let mut approved_24h = 0usize;
        let mut canary_triggers_24h = 0usize;
        let mut unique_agents: HashMap<String, usize> = HashMap::new();
        let mut dangerous_users: HashMap<String, usize> = HashMap::new();

        for entry in self.events.iter() {
            let ev = entry.value();
            unique_agents.entry(ev.agent_id.clone()).and_modify(|c| *c += 1).or_insert(1);

            if ev.timestamp_nanos < cutoff { continue; }

            match &ev.event_type {
                AuditEventType::CommandIntercepted { action, risk_score, .. } => {
                    interceptions_24h += 1;
                    if *risk_score >= 80 {
                        if let Some(user) = ev.event_type.username() {
                            *dangerous_users.entry(user.to_string()).or_insert(0) += 1;
                        }
                    }
                    if action == "blocked" { denied_24h += 1; }
                }
                AuditEventType::CommandApproved { .. } => approved_24h += 1,
                AuditEventType::CommandRejected { .. } => denied_24h += 1,
                AuditEventType::CanaryTriggered { .. } => canary_triggers_24h += 1,
                _ => {}
            }
        }

        let mut top_dangerous_users: Vec<(String, usize)> = dangerous_users.into_iter().collect();
        top_dangerous_users.sort_by(|a, b| b.1.cmp(&a.1));
        top_dangerous_users.truncate(10);

        AuditStats {
            total_events: self.events.len(),
            interceptions_24h,
            denied_24h,
            approved_24h,
            canary_triggers_24h,
            unique_agents: unique_agents.len(),
            top_dangerous_users,
        }
    }

    /// Prune events older than max_age. Returns number of pruned events.
    pub fn prune(&self, max_age: Duration) -> usize {
        let now_nanos = Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64;
        let cutoff = now_nanos.saturating_sub(max_age.as_nanos() as u64);
        let keys: Vec<String> = self.events.iter()
            .filter(|e| e.value().timestamp_nanos < cutoff)
            .map(|e| e.key().clone())
            .collect();
        let count = keys.len();
        for key in keys {
            self.events.remove(&key);
        }
        count
    }

    fn load_journal(&self) -> anyhow::Result<()> {
        if !self.journal_path.exists() { return Ok(()); }
        let file = std::fs::File::open(&self.journal_path)?;
        let reader = std::io::BufReader::new(file);
        for line in reader.lines().flatten() {
            if let Ok(event) = serde_json::from_str::<AuditEvent>(&line) {
                self.events.insert(event.id.clone(), event);
            }
        }
        log::info!("Loaded {} audit events from journal", self.events.len());
        Ok(())
    }

    fn to_cef(&self, event: &AuditEvent) -> String {
        let severity = match &event.event_type {
            AuditEventType::CommandIntercepted { risk_score, .. } => {
                if *risk_score >= 90 { "10" } else if *risk_score >= 70 { "7" } else { "5" }
            }
            AuditEventType::CanaryTriggered { .. } => "10",
            AuditEventType::CommandRejected { .. } => "8",
            AuditEventType::PolicyViolation { .. } => "8",
            _ => "3",
        };
        format!(
            "CEF:0|FlowLink|Shield|1.0|{}|{}|{}|msg={}",
            "Security", // device vendor
            event.event_type.as_str(),
            severity,
            serde_json::to_string(event).unwrap_or_default().replace("|", "\\|"),
        )
    }

    fn to_leef(&self, event: &AuditEvent) -> String {
        format!(
            "LEEF:1.0|FlowLink|Shield|1.0|{}|devTime={} agentId={} cat={}",
            event.event_type.as_str(),
            event.timestamp_iso,
            event.agent_id,
            event.event_type.as_str(),
        )
    }
}

// ═══════════════════════════════════════════════
// Filter & Formats
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Default)]
pub struct AuditFilter {
    pub agent_id: Option<String>,
    pub event_type: Option<String>,
    pub since: Option<u64>,
    pub until: Option<u64>,
    pub min_risk_score: Option<u8>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
pub enum SiemFormat {
    Cef,
    Leef,
    Json,
}

#[derive(Debug, Serialize)]
pub struct AuditStats {
    pub total_events: usize,
    pub interceptions_24h: usize,
    pub denied_24h: usize,
    pub approved_24h: usize,
    pub canary_triggers_24h: usize,
    pub unique_agents: usize,
    pub top_dangerous_users: Vec<(String, usize)>,
}

// ═══════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use flowlink_core::channels::AuditEvent;

    fn make_event(agent_id: &str, ts_nanos: u64) -> AuditEvent {
        let mut ev = AuditEvent::new(agent_id, AuditEventType::CommandIntercepted {
            command: "rm -rf /".into(),
            args: vec!["-rf".into(), "/".into()],
            action: "blocked".into(),
            threat_level: "critical".into(),
            risk_score: 95,
        });
        ev.timestamp_nanos = ts_nanos;
        ev.timestamp_iso = "2024-01-01T00:00:00Z".into();
        ev
    }

    #[test]
    fn test_record_and_query() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.jsonl");
        let store = AuditStore::new(&path);

        store.record(make_event("agent-1", 1000)).unwrap();
        store.record(make_event("agent-2", 2000)).unwrap();

        let results = store.query(&AuditFilter {
            agent_id: Some("agent-1".into()),
            ..Default::default()
        });
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].agent_id, "agent-1");
    }

    #[test]
    fn test_query_by_event_type() {
        let dir = tempfile::tempdir().unwrap();
        let store = AuditStore::new(&dir.path().join("audit.jsonl"));

        store.record(make_event("a1", 1000)).unwrap();
        store.record(AuditEvent::new("a1", AuditEventType::CanaryTriggered {
            path: "/x".into(), accessor: "bob".into(), access_type: "read".into(),
        })).unwrap();

        let results = store.query(&AuditFilter {
            event_type: Some("canary_triggered".into()),
            ..Default::default()
        });
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_query_limit() {
        let dir = tempfile::tempdir().unwrap();
        let store = AuditStore::new(&dir.path().join("audit.jsonl"));
        for i in 0..10 {
            store.record(make_event("a1", i as u64 * 1000)).unwrap();
        }
        let results = store.query(&AuditFilter { limit: Some(3), ..Default::default() });
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_prune() {
        let dir = tempfile::tempdir().unwrap();
        let store = AuditStore::new(&dir.path().join("audit.jsonl"));
        let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64;
        let old = now - 2_000_000_000u64; // 2 seconds ago
        let recent = now - 500_000_000u64; // 0.5 seconds ago
        store.record(make_event("a1", old)).unwrap();
        store.record(make_event("a1", recent)).unwrap();
        let pruned = store.prune(Duration::from_secs(1));
        assert_eq!(pruned, 1);
        assert_eq!(store.events.len(), 1);
    }

    #[test]
    fn test_export_json() {
        let dir = tempfile::tempdir().unwrap();
        let store = AuditStore::new(&dir.path().join("audit.jsonl"));
        store.record(make_event("a1", 1000)).unwrap();
        let json = store.export_json(&AuditFilter::default());
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 1);
    }

    #[test]
    fn test_export_siem_cef() {
        let dir = tempfile::tempdir().unwrap();
        let store = AuditStore::new(&dir.path().join("audit.jsonl"));
        store.record(make_event("a1", 1000)).unwrap();
        let cef = store.export_siem(&SiemFormat::Cef, &AuditFilter::default());
        assert!(cef.contains("CEF:0"));
        assert!(cef.contains("command_intercepted"));
    }

    #[test]
    fn test_export_siem_leef() {
        let dir = tempfile::tempdir().unwrap();
        let store = AuditStore::new(&dir.path().join("audit.jsonl"));
        store.record(make_event("a1", 1000)).unwrap();
        let leef = store.export_siem(&SiemFormat::Leef, &AuditFilter::default());
        assert!(leef.contains("LEEF:1.0"));
    }

    #[test]
    fn test_stats() {
        let dir = tempfile::tempdir().unwrap();
        let store = AuditStore::new(&dir.path().join("audit.jsonl"));
        store.record(make_event("a1", 1000)).unwrap();
        store.record(AuditEvent::new("a2", AuditEventType::CommandApproved {
            command: "ls".into(), approved_by: "admin".into(),
        })).unwrap();
        let stats = store.stats();
        assert_eq!(stats.total_events, 2);
        assert!(stats.unique_agents >= 1);
    }

    #[test]
    fn test_journal_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.jsonl");
        let store1 = AuditStore::new(&path);
        store1.record_sync(make_event("a1", 1000)).unwrap();
        drop(store1);

        let store2 = AuditStore::new(&path);
        assert_eq!(store2.events.len(), 1);
    }

    #[test]
    fn test_risk_score_filter() {
        let dir = tempfile::tempdir().unwrap();
        let store = AuditStore::new(&dir.path().join("audit.jsonl"));
        store.record(make_event("a1", 1000)).unwrap(); // risk_score 95
        store.record(AuditEvent::new("a1", AuditEventType::CommandIntercepted {
            command: "ls".into(), args: vec![], action: "allowed".into(),
            threat_level: "low".into(), risk_score: 10,
        })).unwrap();

        let high_risk = store.query(&AuditFilter { min_risk_score: Some(50), ..Default::default() });
        assert_eq!(high_risk.len(), 1);
    }
}
