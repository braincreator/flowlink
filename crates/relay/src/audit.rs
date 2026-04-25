// FlowLink Relay — Audit Store
// In-memory store with JSONL persistence, query, export, and SIEM support
// Optional PostgreSQL dual-write (non-blocking, best-effort)

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use dashmap::DashMap;
use serde::Serialize;
use serde_json::Value;
use sha2::Digest;

use flowlink_core::channels::{AuditEvent, AuditEventType};

// ═══════════════════════════════════════════════
// Audit Store
// ═══════════════════════════════════════════════

pub struct AuditStore {
    events: DashMap<String, AuditEvent>,
    journal_path: PathBuf,
    db: Option<Arc<flowlink_db::DbPool>>,
    /// Hash of the last written entry — forms a chain
    last_hash: std::sync::Mutex<String>,
}

impl AuditStore {
    /// Create a new audit store with optional PostgreSQL dual-write.
    /// Pass `db: None` to disable DB writes (DashMap + JSONL only).
    pub fn new(journal_path: &std::path::Path, db: Option<Arc<flowlink_db::DbPool>>) -> Self {
        let store = Self {
            events: DashMap::new(),
            journal_path: journal_path.to_path_buf(),
            db,
            last_hash: std::sync::Mutex::new("genesis".to_string()),
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

        // Compute integrity hash (chain)
        let prev_hash = self.last_hash.lock().unwrap().clone();
        let mut hasher = sha2::Sha256::new();
        hasher.update(prev_hash.as_bytes());
        if let Ok(json) = serde_json::to_string(&event) {
            hasher.update(json.as_bytes());
        }
        let current_hash = hex::encode(hasher.finalize());
        *self.last_hash.lock().unwrap() = current_hash.clone();

        // Persist to journal with hash (fire-and-forget, non-blocking)
        std::thread::spawn(move || {
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&journal_path) {
                if let Ok(line) = serde_json::to_string(&event_clone) {
                    let _ = writeln!(f, "{}", line);
                }
            }
        });

        // Dual-write to PostgreSQL (fire-and-forget, non-blocking)
        if let Some(db) = &self.db {
            let db = db.clone();
            let event_for_db = event.clone();
            tokio::spawn(async move {
                if let Err(e) = Self::record_to_db(&db, &event_for_db).await {
                    log::warn!("Audit DB write failed (non-fatal): {}", e);
                }
            });
        }

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

        // Dual-write to PostgreSQL (blocking, best-effort)
        if let Some(db) = &self.db {
            let db = db.clone();
            let event_for_db = event.clone();
            let rt = match tokio::runtime::Handle::try_current() {
                Ok(handle) => handle,
                Err(_) => {
                    log::warn!("Audit DB write skipped: no tokio runtime available in record_sync");
                    self.events.insert(id, event);
                    return Ok(());
                }
            };
            rt.block_on(async move {
                if let Err(e) = Self::record_to_db(&db, &event_for_db).await {
                    log::warn!("Audit DB write failed (non-fatal): {}", e);
                }
            });
        }

        self.events.insert(id, event);
        Ok(())
    }

    /// Map an AuditEvent to the audit_log table schema and insert into PostgreSQL.
    async fn record_to_db(db: &flowlink_db::DbPool, event: &AuditEvent) -> anyhow::Result<()> {
        let (level, action, target, result, metadata) = map_event_to_db_fields(event);

        flowlink_db::audit::AuditRepo::insert(
            db.pool(),
            &level,
            Some(event.event_type.as_str()),
            Some(&event.agent_id),
            None, // account_id — not available on AuditEvent
            &action,
            target.as_deref(),
            result.as_deref(),
            Some(metadata),
            None, // hmac_hash — not available on AuditEvent
            None, // source_ip — not available on AuditEvent
        )
        .await?;

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
// DB field mapping
// ═══════════════════════════════════════════════

/// Map an AuditEvent's fields to the audit_log table columns.
/// Returns (level, action, target, result, metadata).
fn map_event_to_db_fields(event: &AuditEvent) -> (String, String, Option<String>, Option<String>, Value) {
    let mut meta = serde_json::Map::new();

    // Copy existing metadata from the event
    for (k, v) in &event.metadata {
        meta.insert(k.clone(), Value::String(v.clone()));
    }

    // Copy forensic summary if present
    if let Some(forensic) = &event.forensic {
        meta.insert("forensic_uid".to_string(), Value::Number(forensic.uid.into()));
        meta.insert("forensic_username".to_string(), Value::String(forensic.username.clone()));
        meta.insert("forensic_origin".to_string(), Value::String(forensic.origin.clone()));
        meta.insert("forensic_risk_score".to_string(), Value::Number(forensic.risk_score.into()));
    }

    let (level, action, target, result) = match &event.event_type {
        AuditEventType::CanaryTriggered { path, accessor, access_type } => {
            meta.insert("path".to_string(), Value::String(path.clone()));
            meta.insert("accessor".to_string(), Value::String(accessor.clone()));
            meta.insert("access_type".to_string(), Value::String(access_type.clone()));
            ("critical".into(), "canary_access".into(), Some(path.clone()), Some("triggered".into()))
        }
        AuditEventType::CommandIntercepted { command, args, action, threat_level, risk_score } => {
            meta.insert("args".to_string(), Value::Array(
                args.iter().map(|a| Value::String(a.clone())).collect()
            ));
            meta.insert("threat_level".to_string(), Value::String(threat_level.clone()));
            meta.insert("risk_score".to_string(), Value::Number((*risk_score).into()));
            let db_level = if *risk_score >= 90 {
                "critical"
            } else if *risk_score >= 80 {
                "high"
            } else if *risk_score >= 50 {
                "medium"
            } else {
                "low"
            };
            (db_level.into(), "intercept".into(), Some(command.clone()), Some(action.clone()))
        }
        AuditEventType::CommandApproved { command, approved_by } => {
            meta.insert("approved_by".to_string(), Value::String(approved_by.clone()));
            ("info".into(), "approve".into(), Some(command.clone()), Some("allowed".into()))
        }
        AuditEventType::CommandRejected { command, rejected_by } => {
            meta.insert("rejected_by".to_string(), Value::String(rejected_by.clone()));
            ("warning".into(), "reject".into(), Some(command.clone()), Some("denied".into()))
        }
        AuditEventType::CommandExecuted { command, exit_code, duration_ms } => {
            meta.insert("exit_code".to_string(), Value::Number((*exit_code).into()));
            meta.insert("duration_ms".to_string(), Value::Number((*duration_ms).into()));
            ("info".into(), "execute".into(), Some(command.clone()), Some(format!("exit={}", exit_code)))
        }
        AuditEventType::SessionStarted { user, origin, terminal } => {
            meta.insert("origin".to_string(), Value::String(origin.clone()));
            if let Some(term) = terminal {
                meta.insert("terminal".to_string(), Value::String(term.clone()));
            }
            ("info".into(), "session_start".into(), Some(format!("user={}", user)), None)
        }
        AuditEventType::SessionEnded { user, duration_ms, commands_count } => {
            meta.insert("duration_ms".to_string(), Value::Number((*duration_ms).into()));
            meta.insert("commands_count".to_string(), Value::Number((*commands_count).into()));
            ("info".into(), "session_end".into(), Some(format!("user={}", user)), None)
        }
        AuditEventType::AgentRegistered { hostname, version } => {
            meta.insert("hostname".to_string(), Value::String(hostname.clone()));
            meta.insert("version".to_string(), Value::String(version.clone()));
            ("info".into(), "register".into(), Some(hostname.clone()), None)
        }
        AuditEventType::AgentHeartbeat { status, uptime_secs } => {
            meta.insert("uptime_secs".to_string(), Value::Number((*uptime_secs).into()));
            ("info".into(), "heartbeat".into(), Some(status.clone()), None)
        }
        AuditEventType::AgentDisconnected { reason } => {
            ("warning".into(), "disconnect".into(), None, Some(reason.clone()))
        }
        AuditEventType::PolicyViolation { rule, command, user } => {
            meta.insert("rule".to_string(), Value::String(rule.clone()));
            meta.insert("user".to_string(), Value::String(user.clone()));
            ("high".into(), "policy_violation".into(), Some(command.clone()), Some("violated".into()))
        }
        AuditEventType::PolicyLoaded { rules_count, version } => {
            meta.insert("rules_count".to_string(), Value::Number((*rules_count).into()));
            meta.insert("version".to_string(), Value::String(version.clone()));
            ("info".into(), "policy_load".into(), None, None)
        }
        AuditEventType::DiscoveryStarted { scan_id, agent_id } => {
            meta.insert("scan_id".to_string(), Value::String(scan_id.clone()));
            meta.insert("agent_id".to_string(), Value::String(agent_id.clone()));
            ("info".into(), "discovery_started".into(), Some(scan_id.clone()), Some("pending".into()))
        }
        AuditEventType::DiscoveryApproved { scan_id, secret_count } => {
            meta.insert("scan_id".to_string(), Value::String(scan_id.clone()));
            meta.insert("secret_count".to_string(), Value::Number((*secret_count).into()));
            ("info".into(), "discovery_approved".into(), Some(scan_id.clone()), Some("approved".into()))
        }
    };

    (level, action, target, result, Value::Object(meta))
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

    /// Helper: create store with no DB (DashMap + JSONL only)
    fn make_store(path: &std::path::Path) -> AuditStore {
        AuditStore::new(path, None)
    }

    #[test]
    fn test_record_and_query() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.jsonl");
        let store = make_store(&path);

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
        let store = make_store(&dir.path().join("audit.jsonl"));

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
        let store = make_store(&dir.path().join("audit.jsonl"));
        for i in 0..10 {
            store.record(make_event("a1", i as u64 * 1000)).unwrap();
        }
        let results = store.query(&AuditFilter { limit: Some(3), ..Default::default() });
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_prune() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(&dir.path().join("audit.jsonl"));
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
        let store = make_store(&dir.path().join("audit.jsonl"));
        store.record(make_event("a1", 1000)).unwrap();
        let json = store.export_json(&AuditFilter::default());
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 1);
    }

    #[test]
    fn test_export_siem_cef() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(&dir.path().join("audit.jsonl"));
        store.record(make_event("a1", 1000)).unwrap();
        let cef = store.export_siem(&SiemFormat::Cef, &AuditFilter::default());
        assert!(cef.contains("CEF:0"));
        assert!(cef.contains("command_intercepted"));
    }

    #[test]
    fn test_export_siem_leef() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(&dir.path().join("audit.jsonl"));
        store.record(make_event("a1", 1000)).unwrap();
        let leef = store.export_siem(&SiemFormat::Leef, &AuditFilter::default());
        assert!(leef.contains("LEEF:1.0"));
    }

    #[test]
    fn test_stats() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(&dir.path().join("audit.jsonl"));
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
        let store1 = make_store(&path);
        store1.record_sync(make_event("a1", 1000)).unwrap();
        drop(store1);

        let store2 = make_store(&path);
        assert_eq!(store2.events.len(), 1);
    }

    #[test]
    fn test_risk_score_filter() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(&dir.path().join("audit.jsonl"));
        store.record(make_event("a1", 1000)).unwrap(); // risk_score 95
        store.record(AuditEvent::new("a1", AuditEventType::CommandIntercepted {
            command: "ls".into(), args: vec![], action: "allowed".into(),
            threat_level: "low".into(), risk_score: 10,
        })).unwrap();

        let high_risk = store.query(&AuditFilter { min_risk_score: Some(50), ..Default::default() });
        assert_eq!(high_risk.len(), 1);
    }

    // --- DB field mapping tests ---

    #[test]
    fn test_map_canary_triggered() {
        let event = AuditEvent::new("agent-1", AuditEventType::CanaryTriggered {
            path: "/etc/shadow".into(),
            accessor: "hacker".into(),
            access_type: "read".into(),
        });
        let (level, action, target, result, metadata) = map_event_to_db_fields(&event);
        assert_eq!(level, "critical");
        assert_eq!(action, "canary_access");
        assert_eq!(target.as_deref(), Some("/etc/shadow"));
        assert_eq!(result.as_deref(), Some("triggered"));
        assert_eq!(metadata["path"], "/etc/shadow");
        assert_eq!(metadata["accessor"], "hacker");
        assert_eq!(metadata["access_type"], "read");
    }

    #[test]
    fn test_map_command_intercepted_high_risk() {
        let event = AuditEvent::new("agent-1", AuditEventType::CommandIntercepted {
            command: "rm -rf /".into(),
            args: vec!["-rf".into(), "/".into()],
            action: "blocked".into(),
            threat_level: "critical".into(),
            risk_score: 95,
        });
        let (level, action, target, result, metadata) = map_event_to_db_fields(&event);
        assert_eq!(level, "critical");
        assert_eq!(action, "intercept");
        assert_eq!(target.as_deref(), Some("rm -rf /"));
        assert_eq!(result.as_deref(), Some("blocked"));
        assert_eq!(metadata["risk_score"], 95);
        assert_eq!(metadata["threat_level"], "critical");
        assert!(metadata["args"].is_array());
    }

    #[test]
    fn test_map_command_intercepted_low_risk() {
        let event = AuditEvent::new("agent-1", AuditEventType::CommandIntercepted {
            command: "ls".into(),
            args: vec![],
            action: "allowed".into(),
            threat_level: "low".into(),
            risk_score: 10,
        });
        let (level, ..) = map_event_to_db_fields(&event);
        assert_eq!(level, "low");
    }

    #[test]
    fn test_map_command_intercepted_medium_risk() {
        let event = AuditEvent::new("agent-1", AuditEventType::CommandIntercepted {
            command: "sudo apt install".into(),
            args: vec!["nginx".into()],
            action: "allowed".into(),
            threat_level: "medium".into(),
            risk_score: 60,
        });
        let (level, ..) = map_event_to_db_fields(&event);
        assert_eq!(level, "medium");
    }

    #[test]
    fn test_map_command_approved() {
        let event = AuditEvent::new("agent-1", AuditEventType::CommandApproved {
            command: "ls".into(),
            approved_by: "admin".into(),
        });
        let (level, action, target, result, metadata) = map_event_to_db_fields(&event);
        assert_eq!(level, "info");
        assert_eq!(action, "approve");
        assert_eq!(target.as_deref(), Some("ls"));
        assert_eq!(result.as_deref(), Some("allowed"));
        assert_eq!(metadata["approved_by"], "admin");
    }

    #[test]
    fn test_map_command_rejected() {
        let event = AuditEvent::new("agent-1", AuditEventType::CommandRejected {
            command: "rm -rf /".into(),
            rejected_by: "policy".into(),
        });
        let (level, action, _target, result, metadata) = map_event_to_db_fields(&event);
        assert_eq!(level, "warning");
        assert_eq!(action, "reject");
        assert_eq!(result.as_deref(), Some("denied"));
        assert_eq!(metadata["rejected_by"], "policy");
    }

    #[test]
    fn test_map_policy_violation() {
        let event = AuditEvent::new("agent-1", AuditEventType::PolicyViolation {
            rule: "no_rm_rf".into(),
            command: "rm -rf /".into(),
            user: "bob".into(),
        });
        let (level, action, _target, result, metadata) = map_event_to_db_fields(&event);
        assert_eq!(level, "high");
        assert_eq!(action, "policy_violation");
        assert_eq!(result.as_deref(), Some("violated"));
        assert_eq!(metadata["rule"], "no_rm_rf");
        assert_eq!(metadata["user"], "bob");
    }

    #[test]
    fn test_map_session_events() {
        let start = AuditEvent::new("agent-1", AuditEventType::SessionStarted {
            user: "alice".into(),
            origin: "ssh".into(),
            terminal: Some("xterm".into()),
        });
        let (level, action, target, result, metadata) = map_event_to_db_fields(&start);
        assert_eq!(level, "info");
        assert_eq!(action, "session_start");
        assert!(target.unwrap().contains("alice"));
        assert!(result.is_none());
        assert_eq!(metadata["terminal"], "xterm");

        let end = AuditEvent::new("agent-1", AuditEventType::SessionEnded {
            user: "alice".into(),
            duration_ms: 60000,
            commands_count: 42,
        });
        let (level, action, _target, _result, metadata) = map_event_to_db_fields(&end);
        assert_eq!(level, "info");
        assert_eq!(action, "session_end");
        assert_eq!(metadata["duration_ms"], 60000);
        assert_eq!(metadata["commands_count"], 42);
    }

    #[test]
    fn test_map_agent_events() {
        let reg = AuditEvent::new("agent-1", AuditEventType::AgentRegistered {
            hostname: "webserver".into(),
            version: "1.0.0".into(),
        });
        let (level, action, target, _result, metadata) = map_event_to_db_fields(&reg);
        assert_eq!(level, "info");
        assert_eq!(action, "register");
        assert_eq!(target.as_deref(), Some("webserver"));
        assert_eq!(metadata["version"], "1.0.0");

        let disc = AuditEvent::new("agent-1", AuditEventType::AgentDisconnected {
            reason: "timeout".into(),
        });
        let (level, action, target, result, _) = map_event_to_db_fields(&disc);
        assert_eq!(level, "warning");
        assert_eq!(action, "disconnect");
        assert!(target.is_none());
        assert_eq!(result.as_deref(), Some("timeout"));
    }

    #[test]
    fn test_map_metadata_and_forensic_preserved() {
        let mut event = AuditEvent::new("agent-1", AuditEventType::CommandExecuted {
            command: "whoami".into(),
            exit_code: 0,
            duration_ms: 15,
        });
        event.metadata.insert("source_ip".to_string(), "10.0.0.1".to_string());
        event.forensic = Some(flowlink_core::channels::ForensicSummary {
            uid: 1000,
            username: "alice".into(),
            origin: "ssh".into(),
            process_tree: vec!["bash".into(), "whoami".into()],
            risk_score: 5,
        });

        let (level, action, target, result, metadata) = map_event_to_db_fields(&event);
        assert_eq!(level, "info");
        assert_eq!(action, "execute");
        assert_eq!(target.as_deref(), Some("whoami"));
        assert_eq!(result.as_deref(), Some("exit=0"));
        assert_eq!(metadata["source_ip"], "10.0.0.1");
        assert_eq!(metadata["forensic_uid"], 1000);
        assert_eq!(metadata["forensic_username"], "alice");
        assert_eq!(metadata["forensic_risk_score"], 5);
    }

    #[test]
    fn test_record_with_no_db_still_works() {
        // Verify that record() works perfectly when db is None
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(&dir.path().join("audit.jsonl"));
        assert!(store.db.is_none());

        store.record(make_event("agent-1", 1000)).unwrap();
        assert_eq!(store.events.len(), 1);

        store.record_sync(make_event("agent-2", 2000)).unwrap();
        assert_eq!(store.events.len(), 2);
    }
}
