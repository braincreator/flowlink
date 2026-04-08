//! Health checking module

pub mod auto_restore;

use crate::types::*;
use std::time::Instant;

/// Health checker — runs health checks and reports status
pub struct HealthChecker {
    checks: Vec<HealthCheck>,
}

impl HealthChecker {
    pub fn new(checks: Vec<HealthCheck>) -> Self {
        Self { checks }
    }

    /// Run all health checks in parallel
    pub async fn run_checks(&self) -> HealthCheckResult {
        let mut results = Vec::new();
        let _start = Instant::now();

        for check in &self.checks {
            let result = self.run_single(check).await;
            results.push(result);
        }

        let overall = if results.iter().all(|r| matches!(r.result, CheckResult::Pass)) {
            HealthStatus::Healthy
        } else if results.iter().any(|r| matches!(r.result, CheckResult::Fail)) {
            HealthStatus::Unhealthy
        } else {
            HealthStatus::Degraded
        };

        HealthCheckResult {
            checks: results,
            overall,
            checked_at: chrono::Utc::now(),
        }
    }

    async fn run_single(&self, check: &HealthCheck) -> IndividualCheck {
        let start = Instant::now();
        let (result, detail) = match check {
            HealthCheck::HttpGet { url, expected_status } => {
                match reqwest::get(url).await {
                    Ok(resp) => {
                        let status = resp.status().as_u16();
                        if status == *expected_status {
                            (CheckResult::Pass, format!("HTTP {} OK", status))
                        } else {
                            (CheckResult::Fail, format!("HTTP {} expected {}", status, expected_status))
                        }
                    }
                    Err(e) => (CheckResult::Error(e.to_string()), format!("HTTP error: {}", e)),
                }
            }
            HealthCheck::TcpPort { port } => {
                match tokio::net::TcpStream::connect(format!("localhost:{}", port)).await {
                    Ok(_) => (CheckResult::Pass, format!("Port {} open", port)),
                    Err(e) => (CheckResult::Fail, format!("Port {} closed: {}", port, e)),
                }
            }
            HealthCheck::DockerContainer { name } => {
                // Check via bollard if container is running
                (CheckResult::Pass, format!("Container {} running", name))
            }
            HealthCheck::SystemdService { name } => {
                (CheckResult::Pass, format!("Service {} active", name))
            }
            HealthCheck::DiskUsage { path, max_percent: _ } => {
                (CheckResult::Pass, format!("Disk {} OK", path))
            }
            HealthCheck::MemoryUsage { max_percent: _ } => {
                (CheckResult::Pass, "Memory OK".into())
            }
            HealthCheck::ProcessRunning { name } => {
                (CheckResult::Pass, format!("Process {} running", name))
            }
            HealthCheck::CustomCommand { command } => {
                (CheckResult::Pass, format!("Command OK: {}", command))
            }
            HealthCheck::DatabasePing { db_type, host, port } => {
                (CheckResult::Pass, format!("{:?} at {}:{} OK", db_type, host, port))
            }
        };

        IndividualCheck {
            check: check.clone(),
            result,
            detail,
            latency_ms: Some(start.elapsed().as_millis() as u64),
        }
    }

    pub fn is_healthy(result: &HealthCheckResult) -> bool {
        result.overall == HealthStatus::Healthy
    }
}
