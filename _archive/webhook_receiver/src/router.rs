use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::*;

pub struct WebhookRouter {
    pub routing_rules: Vec<RoutingRule>,
    pub service_handlers: HashMap<String, Box<dyn WebhookHandler>>,
    pub rate_limiter: Arc<RwLock<HashMap<String, RateLimiter>>>,
}

impl WebhookRouter {
    pub fn new(routing_rules: Vec<RoutingRule>) -> Self {
        let mut router = Self {
            routing_rules,
            service_handlers: HashMap::new(),
            rate_limiter: Arc::new(RwLock::new(HashMap::new())),
        };
        
        // Register default handlers
        router.register_default_handlers();
        
        router
    }
    
    pub fn register_handler(&mut self, service: String, handler: Box<dyn WebhookHandler>) {
        self.service_handlers.insert(service, handler);
    }
    
    pub async fn route_webhook(&self, webhook: &Webhook) -> Result<()> {
        // Find matching routing rule
        let matching_rules = self.routing_rules.iter()
            .filter(|rule| rule.service == webhook.service && rule.enabled);
        
        for rule in matching_rules {
            // Check rate limits
            if let Some(ref rate_limit) = rule.rate_limit {
                if !self.check_rate_limit(rule.service.as_str(), rate_limit).await {
                    log::warn!("Rate limit exceeded for service {}", rule.service);
                    continue;
                }
            }
            
            // Apply filters
            if self.check_filters(webhook, &rule.filters).await {
                // Execute routing
                self.execute_routing(webhook, rule).await?;
                return Ok(());
            }
        }
        
        // No matching rule found
        Err(anyhow::anyhow!("No matching routing rule for webhook from service {}", webhook.service))
    }
    
    async fn execute_routing(&self, webhook: &Webhook, rule: &RoutingRule) -> Result<()> {
        match rule.target {
            RoutingTarget::FlowLink => {
                self.route_to_flowlink(webhook).await
            }
            RoutingTarget::Discord { channel } => {
                self.route_to_discord(webhook, &channel).await
            }
            RoutingTarget::Slack { channel } => {
                self.route_to_slack(webhook, &channel).await
            }
            RoutingTarget::Webhook { url } => {
                self.route_to_external_webhook(webhook, &url).await
            }
            RoutingTarget::Local { handler } => {
                self.route_to_local_handler(webhook, &handler).await
            }
        }
    }
    
    async fn route_to_flowlink(&self, webhook: &Webhook) -> Result<()> {
        // Send to FlowLink relay
        log::info!("Routing webhook to FlowLink from service {}", webhook.service);
        
        // TODO: Integrate with FlowLink relay
        Ok(())
    }
    
    async fn route_to_discord(&self, webhook: &Webhook, channel: &str) -> Result<()> {
        // Send to Discord
        log::info!("Routing webhook to Discord channel {} from service {}", channel, webhook.service);
        
        // TODO: Integrate with Discord bot
        Ok(())
    }
    
    async fn route_to_slack(&self, webhook: &Webhook, channel: &str) -> Result<()> {
        // Send to Slack
        log::info!("Routing webhook to Slack channel {} from service {}", channel, webhook.service);
        
        // TODO: Integrate with Slack integration
        Ok(())
    }
    
    async fn route_to_external_webhook(&self, webhook: &Webhook, url: &str) -> Result<()> {
        // Forward to external webhook
        log::info!("Forwarding webhook to external URL {} from service {}", url, webhook.service);
        
        let client = reqwest::Client::new();
        let response = client
            .post(url)
            .header("content-type", "application/json")
            .body(webhook.data.clone())
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow::anyhow!("External webhook returned status: {}", response.status()));
        }
        
        Ok(())
    }
    
    async fn route_to_local_handler(&self, webhook: &Webhook, handler: &str) -> Result<()> {
        // Handle locally
        log::info!("Routing webhook to local handler {} from service {}", handler, webhook.service);
        
        if let Some(service_handler) = self.service_handlers.get(&webhook.service) {
            service_handler.handle_webhook(webhook).await
        } else {
            Err(anyhow::anyhow!("No handler found for service {}", webhook.service))
        }
    }
    
    async fn check_rate_limit(&self, service: &str, rate_limit: &RateLimit) -> bool {
        let mut limiter = self.rate_limiter.write().await;
        
        if !limiter.contains_key(service) {
            limiter.insert(service.to_string(), RateLimiter::new(
                rate_limit.requests_per_minute,
                rate_limit.burst_size,
            ));
        }
        
        limiter.get_mut(service).unwrap().check_request()
    }
    
    async fn check_filters(&self, webhook: &Webhook, filters: &[RoutingFilter]) -> bool {
        for filter in filters {
            if !self.apply_filter(webhook, filter).await {
                return false;
            }
        }
        true
    }
    
    async fn apply_filter(&self, webhook: &Webhook, filter: &RoutingFilter) -> bool {
        let value = self.extract_field_value(webhook, &filter.field);
        
        match filter.operator {
            FilterOperator::Equals => value == filter.value,
            FilterOperator::NotEquals => value != filter.value,
            FilterOperator::Contains => value.contains(&filter.value),
            FilterOperator::NotContains => !value.contains(&filter.value),
            FilterOperator::StartsWith => value.starts_with(&filter.value),
            FilterOperator::EndsWith => value.ends_with(&filter.value),
            FilterOperator::Regex => {
                match regex::Regex::new(&filter.value) {
                    Ok(re) => re.is_match(&value),
                    Err(_) => false,
                }
            }
        }
    }
    
    fn extract_field_value(&self, webhook: &Webhook, field: &str) -> String {
        match field {
            "service" => webhook.service.clone(),
            "timestamp" => webhook.timestamp.to_rfc3339(),
            "data" => webhook.data.clone(),
            _ => {
                // Try to extract from JSON data
                if let Ok(json_data) = serde_json::from_str::<serde_json::Value>(&webhook.data) {
                    json_data.get(field)
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string()
                } else {
                    "".to_string()
                }
            }
        }
    }
    
    fn register_default_handlers(&mut self) {
        // GitHub webhook handler
        self.register_handler("github".to_string(), Box::new(GithubWebhookHandler {}));
        
        // GitLab webhook handler
        self.register_handler("gitlab".to_string(), Box::new(GitlabWebhookHandler {}));
        
        // Jenkins webhook handler
        self.register_handler("jenkins".to_string(), Box::new(JenkinsWebhookHandler {}));
        
        // Docker Hub webhook handler
        self.register_handler("docker".to_string(), Box::new(DockerWebhookHandler {}));
        
        // Stripe webhook handler
        self.register_handler("stripe".to_string(), Box::new(StripeWebhookHandler {}));
        
        // Discord webhook handler
        self.register_handler("discord".to_string(), Box::new(DiscordWebhookHandler {}));
        
        // Generic JSON webhook handler
        self.register_handler("generic".to_string(), Box::new(GenericWebhookHandler {}));
    }
}

// Rate limiter implementation
pub struct RateLimiter {
    requests_per_minute: i32,
    burst_size: i32,
    current_count: i32,
    last_reset: chrono::DateTime<chrono::Utc>,
}

impl RateLimiter {
    pub fn new(requests_per_minute: i32, burst_size: i32) -> Self {
        Self {
            requests_per_minute,
            burst_size,
            current_count: 0,
            last_reset: chrono::Utc::now(),
        }
    }
    
    pub fn check_request(&mut self) -> bool {
        let now = chrono::Utc::now();
        let time_since_last_reset = now.signed_duration_since(self.last_reset).num_seconds();
        
        // Reset if more than a minute has passed
        if time_since_last_reset >= 60 {
            self.current_count = 0;
            self.last_reset = now;
        }
        
        // Check if we can allow this request
        if self.current_count < self.burst_size && self.current_count < self.requests_per_minute {
            self.current_count += 1;
            true
        } else {
            false
        }
    }
}

// Webhook handler trait
#[async_trait::async_trait]
pub trait WebhookHandler: Send + Sync {
    async fn handle_webhook(&self, webhook: &Webhook) -> Result<()>;
}

// Default webhook handlers
pub struct GithubWebhookHandler {}

#[async_trait::async_trait]
impl WebhookHandler for GithubWebhookHandler {
    async fn handle_webhook(&self, webhook: &Webhook) -> Result<()> {
        log::info!("Handling GitHub webhook");
        
        // Parse GitHub webhook
        let github_event = webhook.headers.get("x-github-event")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("unknown");
        
        log::debug!("GitHub event type: {}", github_event);
        
        // Handle different event types
        match github_event {
            "push" => {
                let push_data: serde_json::Value = serde_json::from_str(&webhook.data)?;
                log::debug!("Push event: {}", push_data);
            }
            "pull_request" => {
                let pr_data: serde_json::Value = serde_json::from_str(&webhook.data)?;
                log::debug!("Pull request event: {}", pr_data);
            }
            "issues" => {
                let issue_data: serde_json::Value = serde_json::from_str(&webhook.data)?;
                log::debug!("Issues event: {}", issue_data);
            }
            "deployment" => {
                let deploy_data: serde_json::Value = serde_json::from_str(&webhook.data)?;
                log::debug!("Deployment event: {}", deploy_data);
            }
            _ => {
                log::debug!("Unhandled GitHub event type: {}", github_event);
            }
        }
        
        Ok(())
    }
}

pub struct GitlabWebhookHandler {}

#[async_trait::async_trait]
impl WebhookHandler for GitlabWebhookHandler {
    async fn handle_webhook(&self, webhook: &Webhook) -> Result<()> {
        log::info!("Handling GitLab webhook");
        
        // Parse GitLab webhook
        let gitlab_event = webhook.headers.get("x-gitlab-event")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("unknown");
        
        log::debug!("GitLab event type: {}", gitlab_event);
        
        Ok(())
    }
}

pub struct JenkinsWebhookHandler {}

#[async_trait::async_trait]
impl WebhookHandler for JenkinsWebhookHandler {
    async fn handle_webhook(&self, webhook: &Webhook) -> Result<()> {
        log::info!("Handling Jenkins webhook");
        
        // Parse Jenkins webhook
        let jenkins_data: serde_json::Value = serde_json::from_str(&webhook.data)?;
        log::debug!("Jenkins event: {}", jenkins_data);
        
        Ok(())
    }
}

pub struct DockerWebhookHandler {}

#[async_trait::async_trait]
impl WebhookHandler for DockerWebhookHandler {
    async fn handle_webhook(&self, webhook: &Webhook) -> Result<()> {
        log::info!("Handling Docker webhook");
        
        // Parse Docker webhook
        let docker_data: serde_json::Value = serde_json::from_str(&webhook.data)?;
        log::debug!("Docker event: {}", docker_data);
        
        Ok(())
    }
}

pub struct StripeWebhookHandler {}

#[async_trait::async_trait]
impl WebhookHandler for StripeWebhookHandler {
    async fn handle_webhook(&self, webhook: &Webhook) -> Result<()> {
        log::info!("Handling Stripe webhook");
        
        // Parse Stripe webhook
        let stripe_event: serde_json::Value = serde_json::from_str(&webhook.data)?;
        log::debug!("Stripe event: {}", stripe_event);
        
        Ok(())
    }
}

pub struct DiscordWebhookHandler {}

#[async_trait::async_trait]
impl WebhookHandler for DiscordWebhookHandler {
    async fn handle_webhook(&self, webhook: &Webhook) -> Result<()> {
        log::info!("Handling Discord webhook");
        
        // Parse Discord webhook
        let discord_data: serde_json::Value = serde_json::from_str(&webhook.data)?;
        log::debug!("Discord event: {}", discord_data);
        
        Ok(())
    }
}

pub struct GenericWebhookHandler {}

#[async_trait::async_trait]
impl WebhookHandler for GenericWebhookHandler {
    async fn handle_webhook(&self, webhook: &Webhook) -> Result<()> {
        log::info!("Handling generic webhook from {}", webhook.service);
        
        // Store webhook data for later processing
        log::debug!("Webhook data: {}", webhook.data);
        
        Ok(())
    }
}