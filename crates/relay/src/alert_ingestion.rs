// External Alert Ingestion — Prometheus Alertmanager webhook + Zabbix webhook
// Receives alerts from external monitoring, maps to infrastructure nodes, updates health

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use std::collections::HashMap;

use crate::server::AppState;

/// Prometheus Alertmanager webhook payload
#[derive(Debug, Deserialize)]
pub struct AlertmanagerPayload {
    pub status: String,          // firing, resolved
    pub alerts: Vec<AlertmanagerAlert>,
    #[serde(rename = "externalURL")]
    pub external_url: Option<String>,
    pub group_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AlertmanagerAlert {
    pub status: String,          // firing, resolved
    pub labels: HashMap<String, String>,
    pub annotations: HashMap<String, String>,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    #[serde(rename = "generatorURL")]
    pub generator_url: Option<String>,
}

/// Generic webhook alert (Zabbix, Grafana, custom)
#[derive(Debug, Deserialize)]
pub struct GenericAlert {
    pub source: String,          // "zabbix", "grafana", "datadog", "custom"
    pub severity: String,        // "info", "warning", "error", "critical"
    pub host: Option<String>,
    pub service: Option<String>,
    pub message: String,
    pub labels: Option<HashMap<String, String>>,
}

/// POST /api/webhooks/alertmanager
/// Receives Prometheus Alertmanager webhook notifications
pub async fn alertmanager_webhook(
    State(state): State<AppState>,
    Json(payload): Json<AlertmanagerPayload>,
) -> impl IntoResponse {
    let pool = match state.db.as_ref().map(|p| &p.write_pool) {
        Some(p) => p,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false}))).into_response(),
    };

    log::info!("📊 Alertmanager webhook: status={}, alerts={}", payload.status, payload.alerts.len());

    for alert in &payload.alerts {
        let alert_name = alert.labels.get("alertname").cloned().unwrap_or_default();
        let _alert_name_ref = &alert_name;
        let severity = alert.labels.get("severity").cloned()
            .or_else(|| alert.annotations.get("severity").cloned())
            .unwrap_or_else(|| "warning".into());
        let instance = alert.labels.get("instance").cloned();
        let job = alert.labels.get("job").cloned();

        // Map to infrastructure node
        let node_id = instance.as_ref()
            .map(|i| format!("svc-{}", i.replace('.', "-").replace(':', "-")))
            .or_else(|| job.as_ref().map(|j| format!("svc-{}", j)));

        if let Some(nid) = &node_id {
            // Update node health in DB
            let _status = match alert.status.as_str() {
                "firing" => "alert",
                "resolved" => "healthy",
                _ => "degraded",
            };

            let _ = sqlx::query(
                r#"INSERT INTO infra_map_nodes (id, org_id, node_type, data, name, discovered_by, discovered_at, updated_at)
                   VALUES ($1, '00000000-0000-0000-0000-000000000000', 'monitor', $2, $3, 'alertmanager', NOW(), NOW())
                   ON CONFLICT (id) DO UPDATE SET data = $2, updated_at = NOW()"#
            )
            .bind(nid)
            .bind(serde_json::json!({
                "source": "alertmanager",
                "alert_name": alert_name,
                "severity": severity,
                "status": alert.status,
                "labels": alert.labels,
            }).to_string())
            .bind(alert_name.clone())
            .execute(pool)
            .await;

            // Record audit event
            let _ = sqlx::query(
                "INSERT INTO audit_events (event_type, agent_id, org_id, command, metadata, created_at) VALUES ($1, $2, '00000000-0000-0000-0000-000000000000', $3, $4, NOW())"
            )
            .bind(format!("alertmanager_{}", alert.status))
            .bind(nid)
            .bind(alert_name.clone())
            .bind(serde_json::json!({
                "severity": severity,
                "instance": instance,
                "job": job,
                "status": alert.status,
                "annotations": alert.annotations,
            }).to_string())
            .execute(pool)
            .await;
        }

        log::info!(
            "📊 Alert: {} severity={} instance={:?} job={:?} status={}",
            alert_name, severity, instance, job, alert.status
        );
    }

    (StatusCode::OK, Json(serde_json::json!({"ok": true, "processed": payload.alerts.len()}))).into_response()
}

/// POST /api/webhooks/generic-alert
/// Receives generic alerts from Zabbix, Grafana, Datadog, etc.
pub async fn generic_alert_webhook(
    State(state): State<AppState>,
    Json(alert): Json<GenericAlert>,
) -> impl IntoResponse {
    let pool = match state.db.as_ref().map(|p| &p.write_pool) {
        Some(p) => p,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false}))).into_response(),
    };

    log::info!("📊 Generic alert: source={} severity={} host={:?} service={:?}", 
        alert.source, alert.severity, alert.host, alert.service);

    // Map to infrastructure node
    let node_id = alert.host.as_ref()
        .map(|h| format!("svc-{}", h.replace('.', "-").replace(':', "-")))
        .or_else(|| alert.service.as_ref().map(|s| format!("svc-{}", s)));

    if let Some(nid) = &node_id {
        let _ = sqlx::query(
            r#"INSERT INTO audit_events (event_type, agent_id, org_id, command, metadata, created_at) VALUES ($1, $2, '00000000-0000-0000-0000-000000000000', $3, $4, NOW())"#
        )
        .bind(format!("external_{}_{}", alert.source, alert.severity))
        .bind(nid)
        .bind(&alert.message)
        .bind(serde_json::json!({
            "source": alert.source,
            "severity": alert.severity,
            "host": alert.host,
            "service": alert.service,
            "labels": alert.labels,
        }).to_string())
        .execute(pool)
        .await;
    }

    (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alertmanager_payload_deserialize() {
        let payload: AlertmanagerPayload = serde_json::from_str(r#"{
            "status": "firing",
            "alerts": [{
                "status": "firing",
                "labels": {"alertname": "HighCpu", "severity": "warning", "instance": "prod-web-01:9090", "job": "node"},
                "annotations": {"summary": "CPU over 90%"},
                "starts_at": "2026-04-25T20:00:00Z",
                "ends_at": "0001-01-01T00:00:00Z"
            }],
            "externalURL": "http://alertmanager:9093",
            "groupKey": "{}:{}"
        }"#).unwrap();
        assert_eq!(payload.status, "firing");
        assert_eq!(payload.alerts.len(), 1);
        assert_eq!(payload.alerts[0].labels.get("alertname"), Some(&"HighCpu".to_string()));
    }

    #[test]
    fn test_generic_alert_deserialize() {
        let alert: GenericAlert = serde_json::from_str(r#"{
            "source": "zabbix",
            "severity": "error",
            "host": "prod-db-01",
            "service": "postgresql",
            "message": "Connection pool exhausted",
            "labels": {"trigger_id": "12345"}
        }"#).unwrap();
        assert_eq!(alert.source, "zabbix");
        assert_eq!(alert.severity, "error");
        assert_eq!(alert.host, Some("prod-db-01".into()));
    }

    #[test]
    fn test_alertmanager_resolved() {
        let payload: AlertmanagerPayload = serde_json::from_str(r#"{
            "status": "resolved",
            "alerts": [{
                "status": "resolved",
                "labels": {"alertname": "HighMemory", "severity": "critical", "instance": "prod-app-01"},
                "annotations": {},
                "starts_at": "2026-04-25T19:00:00Z",
                "ends_at": "2026-04-25T20:00:00Z"
            }]
        }"#).unwrap();
        assert_eq!(payload.alerts[0].status, "resolved");
    }
}
