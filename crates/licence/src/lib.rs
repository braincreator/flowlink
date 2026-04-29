//! FlowLink Licence Verification
//!
//! Handles licence verification for self-hosted deployments:
//! - Online: verifies against api.flowlink.io
//! - Offline: uses cached licence if within offline_days
//! - Periodic: background check every 24h
//! - Graceful degradation: continues operating within offline window
//!
//! Licence tiers match billing plans from DB:
//! - Free (tier -1): 1 agent, 1 user, 0 ₽
//! - Starter (tier 0): 3 agents, 3 users, 2 990 ₽
//! - Professional (tier 1): 10 agents, 10 users, 19 990 ₽
//! - Business / Scale (tier 2): 30 agents, 30 users, 49 990 ₽
//! - Enterprise (tier 3): unlimited, custom pricing

use std::path::PathBuf;
use std::sync::RwLock;
use async_trait::async_trait;
use flowlink_service_traits::*;
use serde::{Deserialize, Serialize};

/// Licence verification response from cloud
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenceVerifyResponse {
    pub valid: bool,
    pub licence: Option<LicenceInfo>,
    pub message: Option<String>,
}

/// Licence manager — verifies and caches licence
pub struct LicenceManager {
    licence: RwLock<Option<LicenceInfo>>,
    last_check: RwLock<Option<chrono::DateTime<chrono::Utc>>>,
    cache_path: PathBuf,
    check_url: String,
    key: String,
    offline_days: u32,
}

impl LicenceManager {
    pub fn new(key: &str, check_url: &str, cache_path: PathBuf, offline_days: u32) -> Self {
        let mgr = Self {
            licence: RwLock::new(None),
            last_check: RwLock::new(None),
            cache_path,
            check_url: check_url.to_string(),
            key: key.to_string(),
            offline_days,
        };
        let _ = mgr.load_cache();
        mgr
    }

    /// Verify licence against cloud server
    pub async fn verify_online(&self) -> anyhow::Result<LicenceInfo> {
        let client = reqwest::Client::new();
        let resp: LicenceVerifyResponse = client
            .post(&self.check_url)
            .json(&serde_json::json!({ "key": self.key }))
            .send()
            .await?
            .json()
            .await?;

        if let Some(licence) = resp.licence {
            *self.licence.write().unwrap() = Some(licence.clone());
            *self.last_check.write().unwrap() = Some(chrono::Utc::now());
            let _ = self.save_cache(&licence);
            Ok(licence)
        } else {
            anyhow::bail!("Licence invalid: {}", resp.message.unwrap_or_default())
        }
    }

    fn load_cache(&self) -> anyhow::Result<()> {
        if self.cache_path.exists() {
            let data = std::fs::read_to_string(&self.cache_path)?;
            let licence: LicenceInfo = serde_json::from_str(&data)?;
            *self.licence.write().unwrap() = Some(licence);
            log::info!("📦 Loaded cached licence for {}", self.key);
        }
        Ok(())
    }

    fn save_cache(&self, licence: &LicenceInfo) -> anyhow::Result<()> {
        if let Some(parent) = self.cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(licence)?;
        std::fs::write(&self.cache_path, data)?;
        Ok(())
    }

    fn is_within_grace_period(&self) -> bool {
        let last = self.last_check.read().unwrap();
        match *last {
            Some(t) => {
                let elapsed = chrono::Utc::now() - t;
                elapsed < chrono::Duration::days(self.offline_days as i64)
            }
            None => false,
        }
    }
}

#[async_trait]
impl LicenceProvider for LicenceManager {
    async fn verify(&self) -> anyhow::Result<LicenceInfo> {
        match self.verify_online().await {
            Ok(licence) => return Ok(licence),
            Err(e) => log::warn!("Licence online check failed: {}. Trying cache...", e),
        }

        let licence = self.licence.read().unwrap();
        if let Some(ref l) = *licence {
            if self.is_within_grace_period() {
                log::info!("📦 Using cached licence (within {}-day grace)", self.offline_days);
                return Ok(l.clone());
            }
            if l.expires_at > chrono::Utc::now() {
                log::warn!("⚠️ Licence grace period expired, but licence not expired. Allowing.");
                return Ok(l.clone());
            }
        }

        anyhow::bail!("No valid licence found. Contact support@flowlink.io")
    }

    fn has_feature(&self, feature: &str) -> bool {
        self.licence.read().unwrap()
            .as_ref()
            .map(|l| l.features.contains(&feature.to_string()))
            .unwrap_or(false)
    }

    fn max_agents(&self) -> u32 {
        self.licence.read().unwrap()
            .as_ref()
            .map(|l| l.max_agents)
            .unwrap_or(1)
    }

    fn max_users(&self) -> u32 {
        self.licence.read().unwrap()
            .as_ref()
            .map(|l| l.max_users)
            .unwrap_or(1)
    }

    fn is_expired(&self) -> bool {
        self.licence.read().unwrap()
            .as_ref()
            .map(|l| l.expires_at < chrono::Utc::now())
            .unwrap_or(true)
    }

    async fn start_periodic_check(&self, interval_secs: u64) {
        let check_url = self.check_url.clone();
        let key = self.key.clone();
        let cache_path = self.cache_path.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
            loop {
                interval.tick().await;
                let mgr = LicenceManager::new(&key, &check_url, cache_path.clone(), 30);
                match mgr.verify_online().await {
                    Ok(l) => log::info!("✅ Licence verified: {} ({})", l.customer, l.tier),
                    Err(e) => log::warn!("⚠️ Licence check failed: {}", e),
                }
            }
        });
    }
}

/// Licence tiers — must match billing plans in DB
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LicenceTier {
    Free,         // tier -1: 1 agent, 1 user, 0 ₽
    Starter,      // tier 0:  3 agents, 3 users, 2 990 ₽
    Professional, // tier 1:  10 agents, 10 users, 19 990 ₽
    Scale,        // tier 2:  30 agents, 30 users, 49 990 ₽
    Enterprise,   // tier 3:  unlimited, custom pricing
}

impl std::fmt::Display for LicenceTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LicenceTier::Free => write!(f, "Free"),
            LicenceTier::Starter => write!(f, "Starter"),
            LicenceTier::Professional => write!(f, "Pro"),
            LicenceTier::Scale => write!(f, "Business"),
            LicenceTier::Enterprise => write!(f, "Enterprise"),
        }
    }
}
