//! Incident Timeline — reconstructs what happened during an incident.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::Claims;
use crate::server::AppState;

fn require_org(claims: &Claims) -> Result<(String, Uuid), (StatusCode, Json<serde_json::Value>)> {
    match &claims.org_id {
        Some(id) => Ok((id.clone(), id.parse().unwrap_or_default())),
        None => Err((StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "No org"})))),
    }
}

fn require_pool(state: &AppState) -> Result<&sqlx::PgPool, (StatusCode, Json<serde_json::Value>)> {
    state.db.as_ref().map(|db| db.pool()).ok_or_else(|| {
        (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "DB unavailable"})))
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub source: String,
    pub agent_id: Option<String>,
    pub account_id: Option<String>,
    pub action: String,
    pub target: Option<String>,
    pub result: String,
    pub risk_level: String,
    pub details: serde_json::Value,
    pub related_nodes: Vec<String>,
    pub blast_radius: Vec<BlastRadiusEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlastRadiusEntry {
    pub node_id: String,
    pub node_name: String,
    pub node_type: String,
    pub relation: String,
    pub criticality: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentTimeline {
    pub incident_id: String,
    pub org_id: String,
    pub query: TimelineQuery,
    pub entries: Vec<TimelineEntry>,
    pub summary: IncidentSummary,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentSummary {
    pub total_events: usize,
    pub time_range: (DateTime<Utc>, DateTime<Utc>),
    pub agents_involved: Vec<String>,
    pub services_affected: Vec<String>,
    pub commands_executed: usize,
    pub blocked_actions: usize,
    pub approved_actions: usize,
    pub highest_risk: String,
    pub risk_score: u8,
    pub anomalies: Vec<AnomalyRecord>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyRecord {
    pub anomaly_type: String,
    pub description: String,
    pub timestamp: DateTime<Utc>,
    pub severity: String,
    pub related_entries: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineQuery {
    pub agent_id: Option<String>,
    pub account_id: Option<String>,
    pub service_name: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub min_risk: Option<String>,
    pub limit: Option<i64>,
    pub include_blast_radius: Option<bool>,
}

/// GET /api/v1/forensics/timeline
pub async fn get_timeline(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(params): Query<TimelineQuery>,
) -> impl IntoResponse {
    let pool = match require_pool(&state) { Ok(p) => p, Err(e) => return e.into_response() };
    let (org_str, org_uuid) = match require_org(&claims) { Ok(v) => v, Err(e) => return e.into_response() };

    let limit = params.limit.unwrap_or(500).min(2000);
    let from = params.from.unwrap_or_else(|| Utc::now() - chrono::Duration::hours(24));
    let to = params.to.unwrap_or_else(|| Utc::now());

    let mut entries: Vec<TimelineEntry> = Vec::new();

    // 1. audit_log
    if let Ok(rows) = sqlx::query_as::<_, (DateTime<Utc>, String, Option<String>, Option<String>, String, Option<String>, Option<String>, Option<serde_json::Value>)>(
        "SELECT timestamp, level, agent_id, account_id, action, target, result, metadata FROM audit_log WHERE org_id = $1 AND timestamp BETWEEN $2 AND $3 ORDER BY timestamp DESC LIMIT $4"
    )
    .bind(org_uuid).bind(from).bind(to).bind(limit)
    .fetch_all(pool).await
    {
        for (ts, level, aid, acid, action, target, result, meta) in rows {
            let agent_match = params.agent_id.as_deref() == aid.as_deref();
            let account_match = params.account_id.as_deref() == acid.as_deref();
            let no_filter = params.agent_id.is_none() && params.account_id.is_none() && params.service_name.is_none();
            if !no_filter && !agent_match && !account_match { continue; }

            entries.push(TimelineEntry {
                timestamp: ts, event_type: "audit".into(), source: "audit_log".into(),
                agent_id: aid, account_id: acid, action, target,
                result: result.unwrap_or_else(|| "unknown".into()),
                risk_level: level.clone(), details: meta.unwrap_or(serde_json::json!({})),
                related_nodes: vec![], blast_radius: vec![],
            });
        }
    }

    // 2. command_history
    if let Ok(rows) = sqlx::query_as::<_, (DateTime<Utc>, String, String, Option<String>, Option<i32>, Option<i32>, String, String)>(
        "SELECT executed_at, agent_id, command, args, exit_code, duration_ms, shield_result, shield_risk FROM command_history WHERE org_id = $1 AND executed_at BETWEEN $2 AND $3 ORDER BY executed_at DESC LIMIT $4"
    )
    .bind(org_uuid).bind(from).bind(to).bind(limit)
    .fetch_all(pool).await
    {
        for (ts, aid, cmd, args, exit_code, dur_ms, shield_result, shield_risk) in rows {
            let agent_match = params.agent_id.as_deref() == Some(aid.as_str());
            let no_filter = params.agent_id.is_none() && params.account_id.is_none() && params.service_name.is_none();
            if !no_filter && !agent_match { continue; }

            let risk = match shield_risk.as_str() { "critical" | "high" => "high", "medium" => "medium", _ => "low" };

            entries.push(TimelineEntry {
                timestamp: ts, event_type: "command".into(), source: "command_history".into(),
                agent_id: Some(aid), account_id: None, action: cmd, target: args,
                result: shield_result, risk_level: risk.into(),
                details: serde_json::json!({"exit_code": exit_code, "duration_ms": dur_ms}),
                related_nodes: vec![], blast_radius: vec![],
            });
        }
    }

    entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    entries.truncate(limit as usize);

    // 3. Blast radius
    if params.include_blast_radius.unwrap_or(true) {
        for entry in &mut entries {
            if let Some(ref agent_id) = entry.agent_id {
                if let Ok(nodes) = sqlx::query_as::<_, (String, String, String, String)>(
                    "SELECT n.id, n.name, n.node_type, COALESCE(n.criticality, 'medium') FROM infra_map_nodes n WHERE n.org_id = $1 AND n.discovered_by = $2 LIMIT 20"
                ).bind(org_uuid).bind(agent_id).fetch_all(pool).await {
                    entry.related_nodes = nodes.iter().map(|(id, _, _, _)| id.clone()).collect();
                    for (id, name, ntype, crit) in nodes {
                        if let Ok(edges) = sqlx::query_as::<_, (String, String, String, String)>(
                            "SELECT n.id, n.name, n.node_type, COALESCE(n.criticality, 'medium') FROM infra_map_edges e JOIN infra_map_nodes n ON (e.to_id = n.id OR e.from_id = n.id) WHERE e.org_id = $1 AND (e.from_id = $2 OR e.to_id = $2) AND n.id != $2 LIMIT 10"
                        ).bind(org_uuid).bind(&id).fetch_all(pool).await {
                            for (eid, ename, etype, ecrit) in edges {
                                entry.blast_radius.push(BlastRadiusEntry { node_id: eid, node_name: ename, node_type: etype, relation: "connected".into(), criticality: ecrit });
                            }
                        }
                    }
                }
            }
        }
    }

    let anomalies = detect_anomalies(&entries);
    let summary = build_summary(&entries, &anomalies, from, to);

    let timeline = IncidentTimeline {
        incident_id: format!("INC-{}", Utc::now().format("%Y%m%d%H%M%S")),
        org_id: org_str,
        query: params,
        entries, summary,
        generated_at: Utc::now(),
    };

    Json(timeline).into_response()
}

#[derive(Debug, Deserialize)]
pub struct ReconstructParams { pub hours: Option<i64> }

/// GET /api/v1/forensics/reconstruct/{agent_id}
pub async fn reconstruct_agent(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(agent_id): Path<String>,
    Query(params): Query<ReconstructParams>,
) -> impl IntoResponse {
    let pool = match require_pool(&state) { Ok(p) => p, Err(e) => return e.into_response() };
    let (org_str, org_uuid) = match require_org(&claims) { Ok(v) => v, Err(e) => return e.into_response() };

    let hours = params.hours.unwrap_or(24);
    let from = Utc::now() - chrono::Duration::hours(hours);

    let commands = sqlx::query_as::<_, (DateTime<Utc>, String, Option<String>, Option<i32>, Option<i32>, String, String)>(
        "SELECT executed_at, command, args, exit_code, duration_ms, shield_result, shield_risk FROM command_history WHERE org_id = $1 AND agent_id = $2 AND executed_at > $3 ORDER BY executed_at ASC"
    ).bind(org_uuid).bind(&agent_id).bind(from).fetch_all(pool).await.unwrap_or_default();

    let audits = sqlx::query_as::<_, (DateTime<Utc>, String, String, Option<String>, Option<String>, Option<serde_json::Value>)>(
        "SELECT timestamp, level, action, target, result, metadata FROM audit_log WHERE org_id = $1 AND agent_id = $2 AND timestamp > $3 ORDER BY timestamp ASC"
    ).bind(org_uuid).bind(&agent_id).bind(from).fetch_all(pool).await.unwrap_or_default();

    let infra_touched = sqlx::query_as::<_, (String, String)>(
        "SELECT DISTINCT n.name, n.node_type FROM infra_map_nodes n JOIN infra_map_snapshots s ON n.org_id = s.org_id WHERE n.org_id = $1 AND s.agent_id = $2 AND s.created_at > $3"
    ).bind(org_uuid).bind(&agent_id).bind(from).fetch_all(pool).await.unwrap_or_default();

    let mut scenario: Vec<serde_json::Value> = Vec::new();
    for (ts, cmd, args, exit_code, dur, result, risk) in &commands {
        scenario.push(serde_json::json!({"timestamp": ts, "type": "command", "command": cmd, "args": args, "exit_code": exit_code, "duration_ms": dur, "shield_result": result, "risk": risk}));
    }
    for (ts, level, action, target, result, meta) in &audits {
        scenario.push(serde_json::json!({"timestamp": ts, "type": "audit", "level": level, "action": action, "target": target, "result": result, "metadata": meta}));
    }
    scenario.sort_by(|a, b| {
        let ta = a.get("timestamp").and_then(|v| v.as_str()).unwrap_or("");
        let tb = b.get("timestamp").and_then(|v| v.as_str()).unwrap_or("");
        ta.cmp(tb)
    });

    let services: Vec<String> = infra_touched.into_iter().map(|(n, _)| n).collect();

    Json(serde_json::json!({
        "agent_id": agent_id, "org_id": org_str, "time_window_hours": hours,
        "total_commands": commands.len(), "total_audit_events": audits.len(),
        "services_touched": services,
        "blocked_commands": commands.iter().filter(|(_, _, _, _, _, r, _)| r == "blocked").count(),
        "high_risk_commands": commands.iter().filter(|(_, _, _, _, _, _, r)| r == "high" || r == "critical").count(),
        "scenario": scenario,
        "generated_at": Utc::now(),
    })).into_response()
}

fn detect_anomalies(entries: &[TimelineEntry]) -> Vec<AnomalyRecord> {
    let mut anomalies = Vec::new();
    for (i, entry) in entries.iter().enumerate() {
        let hour: u32 = entry.timestamp.format("%H").to_string().parse().unwrap_or(12);
        if (hour >= 22 || hour < 6) && (entry.risk_level == "high" || entry.risk_level == "critical") {
            anomalies.push(AnomalyRecord { anomaly_type: "off_hours".into(), description: format!("High-risk at {}: {}", entry.timestamp.format("%H:%M UTC"), entry.action), timestamp: entry.timestamp, severity: "medium".into(), related_entries: vec![i] });
        }
        if entry.action.contains("sudo") || entry.action.contains("chmod 777") {
            anomalies.push(AnomalyRecord { anomaly_type: "privilege_escalation".into(), description: format!("Privilege escalation: {}", entry.action), timestamp: entry.timestamp, severity: "high".into(), related_entries: vec![i] });
        }
        if entry.action.contains("scp") || entry.action.contains("rsync") {
            anomalies.push(AnomalyRecord { anomaly_type: "data_exfil_risk".into(), description: format!("Data transfer: {}", entry.action), timestamp: entry.timestamp, severity: "medium".into(), related_entries: vec![i] });
        }
        if i > 0 {
            if let (Some(a1), Some(a2)) = (&entry.agent_id, &entries[i-1].agent_id) {
                if a1 == a2 && (entry.timestamp - entries[i-1].timestamp).num_minutes().abs() < 5 && entry.target != entries[i-1].target {
                    anomalies.push(AnomalyRecord { anomaly_type: "lateral_movement".into(), description: format!("Rapid multi-target by {}", a1), timestamp: entry.timestamp, severity: "high".into(), related_entries: vec![i, i-1] });
                }
            }
        }
    }
    anomalies
}

fn build_summary(entries: &[TimelineEntry], anomalies: &[AnomalyRecord], from: DateTime<Utc>, to: DateTime<Utc>) -> IncidentSummary {
    let agents: Vec<String> = entries.iter().filter_map(|e| e.agent_id.clone()).collect::<std::collections::HashSet<_>>().into_iter().collect();
    let services: Vec<String> = entries.iter().flat_map(|e| e.related_nodes.clone()).collect::<std::collections::HashSet<_>>().into_iter().collect();
    let blocked = entries.iter().filter(|e| e.result == "blocked").count();
    let approved = entries.iter().filter(|e| e.result == "approved").count();
    let highest = entries.iter().map(|e| match e.risk_level.as_str() { "critical" => 4, "high" => 3, "medium" => 2, "low" => 1, _ => 0 }).max().map(|v| match v { 4 => "critical", 3 => "high", 2 => "medium", 1 => "low", _ => "info" }).unwrap_or("info").to_string();
    let risk_score = std::cmp::min(100, (entries.len() as u8 / 2).min(50) + (blocked as u8 * 5).min(30) + (anomalies.len() as u8 * 10).min(20));

    let mut recs = Vec::new();
    if blocked > 0 { recs.push(format!("{} blocked — review shield rules", blocked)); }
    if anomalies.iter().any(|a| a.anomaly_type == "privilege_escalation") { recs.push("Privilege escalation — tighten sudo policies".into()); }
    if anomalies.iter().any(|a| a.anomaly_type == "lateral_movement") { recs.push("Lateral movement — review agent scope".into()); }
    if recs.is_empty() { recs.push("No significant anomalies".into()); }

        IncidentSummary {
        total_events: entries.len(), time_range: (from, to),
        agents_involved: agents, services_affected: services,
        commands_executed: entries.iter().filter(|e| e.event_type == "command").count(),
        blocked_actions: blocked, approved_actions: approved,
        highest_risk: highest, risk_score, anomalies: anomalies.to_vec(), recommendations: recs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn make_entry(ts: &str, event_type: &str, action: &str, risk: &str, result: &str, agent_id: Option<&str>) -> TimelineEntry {
        TimelineEntry {
            timestamp: Utc.with_ymd_and_hms(2026, 4, 25, 
                ts[0..2].parse().unwrap(), ts[3..5].parse().unwrap(), 0).unwrap(),
            event_type: event_type.into(), source: "test".into(),
            agent_id: agent_id.map(String::from), account_id: None,
            action: action.into(), target: None, result: result.into(),
            risk_level: risk.into(), details: serde_json::json!({}),
            related_nodes: vec![], blast_radius: vec![],
        }
    }

    #[test]
    fn test_detect_off_hours() {
        let entries = vec![
            make_entry("23:30", "command", "rm -rf /", "high", "blocked", Some("agent-1")),
        ];
        let anomalies = detect_anomalies(&entries);
        assert_eq!(anomalies.len(), 1);
        assert_eq!(anomalies[0].anomaly_type, "off_hours");
    }

    #[test]
    fn test_detect_privilege_escalation() {
        let entries = vec![
            make_entry("10:00", "command", "sudo chmod 777 /etc", "high", "allowed", Some("agent-1")),
        ];
        let anomalies = detect_anomalies(&entries);
        assert!(anomalies.iter().any(|a| a.anomaly_type == "privilege_escalation"));
    }

    #[test]
    fn test_detect_lateral_movement() {
        let base = Utc.with_ymd_and_hms(2026, 4, 25, 10, 0, 0).unwrap();
        let entries = vec![
            TimelineEntry {
                timestamp: base, event_type: "command".into(), source: "test".into(),
                agent_id: Some("agent-1".into()), account_id: None,
                action: "ssh".into(), target: Some("host-a".into()), result: "allowed".into(),
                risk_level: "low".into(), details: serde_json::json!({}),
                related_nodes: vec![], blast_radius: vec![],
            },
            TimelineEntry {
                timestamp: base + chrono::Duration::minutes(1), event_type: "command".into(), source: "test".into(),
                agent_id: Some("agent-1".into()), account_id: None,
                action: "ssh".into(), target: Some("host-b".into()), result: "allowed".into(),
                risk_level: "low".into(), details: serde_json::json!({}),
                related_nodes: vec![], blast_radius: vec![],
            },
        ];
        let anomalies = detect_anomalies(&entries);
        assert!(anomalies.iter().any(|a| a.anomaly_type == "lateral_movement"));
    }

    #[test]
    fn test_detect_data_exfil() {
        let entries = vec![
            make_entry("10:00", "command", "scp /etc/secrets user@external:", "medium", "blocked", Some("agent-1")),
        ];
        let anomalies = detect_anomalies(&entries);
        assert!(anomalies.iter().any(|a| a.anomaly_type == "data_exfil_risk"));
    }

    #[test]
    fn test_no_anomalies_normal_hours() {
        let entries = vec![
            make_entry("10:00", "audit", "login", "info", "allowed", Some("agent-1")),
        ];
        let anomalies = detect_anomalies(&entries);
        assert!(anomalies.is_empty());
    }

    #[test]
    fn test_build_summary_basic() {
        let from = Utc.with_ymd_and_hms(2026, 4, 25, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2026, 4, 25, 23, 59, 59).unwrap();
        let entries = vec![
            make_entry("10:00", "command", "ls", "low", "allowed", Some("a1")),
            make_entry("11:00", "command", "rm", "high", "blocked", Some("a2")),
        ];
        let anomalies = vec![];
        let summary = build_summary(&entries, &anomalies, from, to);
        assert_eq!(summary.total_events, 2);
        assert_eq!(summary.blocked_actions, 1);
        assert_eq!(summary.approved_actions, 0);
        assert_eq!(summary.commands_executed, 2);
        assert_eq!(summary.highest_risk, "high");
        assert_eq!(summary.agents_involved.len(), 2);
    }

    #[test]
    fn test_risk_score_calculation() {
        let from = Utc.with_ymd_and_hms(2026, 4, 25, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2026, 4, 25, 23, 59, 59).unwrap();
        // 10 entries, 2 blocked → score = min(50, 10/2=5) + min(30, 2*5=10) = 15
        let entries: Vec<TimelineEntry> = (0..10).map(|i| {
            make_entry("10:00", "command", &format!("cmd-{}", i), "low",
                if i < 2 { "blocked" } else { "allowed" }, Some("a1"))
        }).collect();
        let summary = build_summary(&entries, &[], from, to);
        assert_eq!(summary.risk_score, 15);
    }

    #[test]
    fn test_risk_score_max_100() {
        let from = Utc.with_ymd_and_hms(2026, 4, 25, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2026, 4, 25, 23, 59, 59).unwrap();
        let entries: Vec<TimelineEntry> = (0..200).map(|i| {
            make_entry("10:00", "command", &format!("cmd-{}", i), "critical", "blocked", Some("a1"))
        }).collect();
        let anomaly = AnomalyRecord {
            anomaly_type: "privilege_escalation".into(),
            description: "test".into(),
            timestamp: from, severity: "high".into(), related_entries: vec![],
        };
        let summary = build_summary(&entries, &std::iter::repeat(anomaly).take(5).collect::<Vec<_>>(), from, to);
        assert_eq!(summary.risk_score, 100);
    }
}
