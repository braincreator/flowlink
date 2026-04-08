//! Tochka Bank payment gateway client
//!
//! Two payment modes:
//! 1. **Subscriptions API** — рекуррентные автосписания (основная подписка)
//!    - SBP/карта привязывается один раз
//!    - Автосписание по расписанию (месяц, квартал, год)
//! 2. **Acquiring API** — разовые платежи (доп.услуги, top-up)
//!    - SBP QR / hosted checkout для карт
//!
//! API docs: https://enter.tochka.com/doc/api/v2/

use anyhow::{Result, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::payment::{PaymentStatus, SbpConfig};

// ---------------------------------------------------------------------------
// Subscription periods
// ---------------------------------------------------------------------------

/// Billing period for subscriptions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BillingPeriod {
    /// Monthly
    Month,
    /// Quarterly (3 months)
    Quarter,
    /// Yearly (12 months)
    Year,
    /// Custom days
    Days(u16),
}

impl BillingPeriod {
    /// Period in days
    pub fn days(&self) -> u16 {
        match self {
            BillingPeriod::Month => 30,
            BillingPeriod::Quarter => 90,
            BillingPeriod::Year => 365,
            BillingPeriod::Days(d) => *d,
        }
    }

    /// Human-readable name in Russian
    pub fn display_name(&self) -> &str {
        match self {
            BillingPeriod::Month => "Месяц",
            BillingPeriod::Quarter => "Квартал",
            BillingPeriod::Year => "Год",
            BillingPeriod::Days(_) => "Пользовательский",
        }
    }

    /// Short key for API/DB
    pub fn as_str(&self) -> &str {
        match self {
            BillingPeriod::Month => "month",
            BillingPeriod::Quarter => "quarter",
            BillingPeriod::Year => "year",
            BillingPeriod::Days(_) => "custom",
        }
    }

    /// Parse from string
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s {
            "month" | "monthly" => Some(BillingPeriod::Month),
            "quarter" | "quarterly" => Some(BillingPeriod::Quarter),
            "year" | "yearly" | "annual" => Some(BillingPeriod::Year),
            _ => None,
        }
    }

    /// Chrono Duration
    pub fn to_duration(&self) -> chrono::Duration {
        chrono::Duration::days(self.days() as i64)
    }

    /// Price multiplier relative to monthly (for annual discount etc.)
    pub fn price_multiplier(&self) -> f64 {
        match self {
            BillingPeriod::Month => 1.0,
            BillingPeriod::Quarter => 2.7,       // 10% discount vs 3x monthly
            BillingPeriod::Year => 10.0,         // 17% discount vs 12x monthly
            BillingPeriod::Days(d) => *d as f64 / 30.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Subscription types
// ---------------------------------------------------------------------------

/// Subscription status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubscriptionStatus {
    /// Active, auto-billing
    Active,
    /// Paused by user or admin
    Paused,
    /// Past due — payment failed, retry pending
    PastDue,
    /// Cancelled — no more billing
    Cancelled,
    /// Trial period
    Trial,
    /// Expired
    Expired,
}

impl SubscriptionStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::PastDue => "past_due",
            Self::Cancelled => "cancelled",
            Self::Trial => "trial",
            Self::Expired => "expired",
        }
    }

    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "active" => Some(Self::Active),
            "paused" | "suspended" => Some(Self::Paused),
            "past_due" | "overdue" => Some(Self::PastDue),
            "cancelled" | "canceled" | "terminated" => Some(Self::Cancelled),
            "trial" => Some(Self::Trial),
            "expired" => Some(Self::Expired),
            _ => None,
        }
    }

    /// Whether the subscription should still provide service
    pub fn is_serving(&self) -> bool {
        matches!(self, Self::Active | Self::Trial | Self::PastDue)
    }
}

/// Create subscription request
#[derive(Debug, Serialize)]
pub struct CreateSubscriptionRequest {
    /// Customer account ID (our account_id)
    pub customer_id: String,
    /// Plan ID
    pub plan_id: String,
    /// Billing period
    pub period: BillingPeriod,
    /// Amount in kopecks per period
    pub amount: u64,
    /// Payment method for the subscription
    pub payment_method: SubscriptionPaymentMethod,
    /// Description
    pub description: String,
    /// Start date (ISO 8601). None = immediately
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date: Option<DateTime<Utc>>,
    /// Trial days (0 = no trial)
    #[serde(default)]
    pub trial_days: u16,
}

/// Payment method for subscription
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SubscriptionPaymentMethod {
    /// SBP auto-payment
    Sbp {
        /// Phone number for SBP binding
        phone: String,
    },
    /// Bank card (Mir/Visa via acquiring)
    Card {
        /// Saved card token (after first payment)
        #[serde(skip_serializing_if = "Option::is_none")]
        card_token: Option<String>,
    },
}

/// Subscription response from Tochka
#[derive(Debug, Deserialize)]
pub struct SubscriptionResponse {
    /// Subscription ID from Tochka
    pub subscription_id: String,
    /// Our customer ID
    pub customer_id: String,
    /// Plan ID
    pub plan_id: String,
    /// Status
    pub status: String,
    /// Current period
    pub period: String,
    /// Amount in kopecks
    pub amount: u64,
    /// Next billing date
    #[serde(default)]
    pub next_billing_date: Option<DateTime<Utc>>,
    /// Current period start
    #[serde(default)]
    pub current_period_start: Option<DateTime<Utc>>,
    /// Current period end
    #[serde(default)]
    pub current_period_end: Option<DateTime<Utc>>,
    /// Error
    #[serde(default)]
    pub error_code: Option<String>,
    #[serde(default)]
    pub error_description: Option<String>,
}

/// Callback notification from Tochka (subscriptions)
#[derive(Debug, Deserialize)]
pub struct SubscriptionCallback {
    /// Subscription ID
    pub subscription_id: String,
    /// Event type: "created", "renewed", "payment_failed", "cancelled", "paused", "resumed"
    pub event: String,
    /// New status
    pub status: String,
    /// Payment ID (for renewed/failed events)
    #[serde(default)]
    pub payment_id: Option<String>,
    /// Amount charged (kopecks)
    #[serde(default)]
    pub amount: Option<u64>,
    /// Error reason (for payment_failed)
    #[serde(default)]
    pub failure_reason: Option<String>,
    /// Timestamp
    #[serde(default)]
    pub timestamp: Option<DateTime<Utc>>,
    /// HMAC signature
    #[serde(default)]
    pub signature: Option<String>,
}

// ---------------------------------------------------------------------------
// Acquiring types (one-time payments)
// ---------------------------------------------------------------------------

/// SBP payment request
#[derive(Debug, Serialize)]
pub struct SbpPaymentRequest {
    pub amount: u64,
    pub order: String,
    pub description: String,
    #[serde(rename = "paymentType")]
    pub payment_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_phone: Option<String>,
}

/// SBP payment response
#[derive(Debug, Deserialize)]
pub struct SbpPaymentResponse {
    pub payment_id: String,
    pub order: String,
    pub status: String,
    #[serde(default)]
    pub payment_url: Option<String>,
    #[serde(default)]
    pub qr_code: Option<String>,
    #[serde(default)]
    pub error_code: Option<String>,
    #[serde(default)]
    pub error_description: Option<String>,
}

/// Card payment request
#[derive(Debug, Serialize)]
pub struct CardPaymentRequest {
    pub amount: u64,
    pub order: String,
    pub description: String,
    #[serde(default = "default_ru")]
    pub language: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_url: Option<String>,
}

fn default_ru() -> String { "ru".to_string() }

/// Card payment response
#[derive(Debug, Deserialize)]
pub struct CardPaymentResponse {
    pub payment_id: String,
    pub order: String,
    pub status: String,
    #[serde(default)]
    pub payment_url: Option<String>,
    #[serde(default)]
    pub error_code: Option<String>,
    #[serde(default)]
    pub error_description: Option<String>,
}

/// Payment status query response
#[derive(Debug, Deserialize)]
pub struct PaymentStatusResponse {
    pub payment_id: String,
    pub order: String,
    pub status: String,
    #[serde(default)]
    pub amount: Option<u64>,
    #[serde(default)]
    pub error_code: Option<String>,
    #[serde(default)]
    pub error_description: Option<String>,
}

/// Refund request/response
#[derive(Debug, Serialize)]
pub struct RefundRequest {
    pub payment_id: String,
    pub amount: u64,
}

#[derive(Debug, Deserialize)]
pub struct RefundResponse {
    pub refund_id: String,
    pub status: String,
    #[serde(default)]
    pub error_code: Option<String>,
    #[serde(default)]
    pub error_description: Option<String>,
}

// ---------------------------------------------------------------------------
// HTTP backend trait + implementations
// ---------------------------------------------------------------------------

#[async_trait::async_trait]
pub trait TochkaHttp: Send + Sync {
    async fn post(&self, path: &str, body: &str) -> Result<String>;
    async fn get(&self, path: &str) -> Result<String>;
    async fn delete(&self, path: &str) -> Result<String>;
}

/// Real HTTP backend using reqwest (only with tochka-live feature)
#[cfg(feature = "tochka-live")]
pub struct ReqwestBackend {
    client: reqwest::Client,
    base_url: String,
}

#[cfg(feature = "tochka-live")]
impl ReqwestBackend {
    pub fn new(base_url: String) -> Self {
        Self { client: reqwest::Client::new(), base_url }
    }
}

#[cfg(feature = "tochka-live")]
#[async_trait::async_trait]
impl TochkaHttp for ReqwestBackend {
    async fn post(&self, path: &str, body: &str) -> Result<String> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.post(&url)
            .header("Content-Type", "application/json")
            .body(body.to_string())
            .send().await?;
        Ok(resp.text().await?)
    }

    async fn get(&self, path: &str) -> Result<String> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.get(&url).send().await?;
        Ok(resp.text().await?)
    }

    async fn delete(&self, path: &str) -> Result<String> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.delete(&url).send().await?;
        Ok(resp.text().await?)
    }
}

// ---------------------------------------------------------------------------
// HMAC-SHA256 (no external dependency)
// ---------------------------------------------------------------------------

fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    use sha2::Digest;
    // HMAC-SHA256 per RFC 2104
    let block_size = 64;
    let mut key_padded = [0u8; 64];
    if key.len() > block_size {
        let hash = sha2::Sha256::digest(key);
        key_padded[..32].copy_from_slice(&hash);
    } else {
        key_padded[..key.len()].copy_from_slice(key);
    }

    let mut o_key_pad = [0x5cu8; 64];
    let mut i_key_pad = [0x36u8; 64];
    for i in 0..block_size {
        o_key_pad[i] ^= key_padded[i];
        i_key_pad[i] ^= key_padded[i];
    }

    let mut inner = Vec::with_capacity(block_size + message.len());
    inner.extend_from_slice(&i_key_pad);
    inner.extend_from_slice(message);
    let inner_hash = sha2::Sha256::digest(&inner);

    let mut outer = Vec::with_capacity(block_size + 32);
    outer.extend_from_slice(&o_key_pad);
    outer.extend_from_slice(&inner_hash);

    sha2::Sha256::digest(&outer).into()
}

fn constant_time_compare(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() { return false; }
    let mut result: u8 = 0;
    for (x, y) in a.iter().zip(b) { result |= x ^ y; }
    result == 0
}

// ---------------------------------------------------------------------------
// TochkaClient
// ---------------------------------------------------------------------------

/// Tochka Bank API client — subscriptions + acquiring
pub struct TochkaClient {
    config: SbpConfig,
    http: Box<dyn TochkaHttp>,
}

impl TochkaClient {
    /// Create a new Tochka client with real HTTP backend
    #[cfg(feature = "tochka-live")]
    pub fn new(config: SbpConfig) -> Self {
        Self {
            http: Box::new(ReqwestBackend::new(
                "https://enter.tochka.com/api/v2".to_string()
            )),
            config,
        }
    }

    pub fn with_http(config: SbpConfig, http: Box<dyn TochkaHttp>) -> Self {
        Self { config, http }
    }

    pub fn config(&self) -> &SbpConfig { &self.config }

    // ---- Signing / verification ----

    pub fn sign(&self, payload: &str) -> String {
        let hash = hmac_sha256(self.config.secret_key.as_bytes(), payload.as_bytes());
        hash.iter().map(|b| format!("{:02x}", b)).collect()
    }

    pub fn verify_signature(&self, payload: &str, signature: &str) -> bool {
        let expected = self.sign(payload);
        constant_time_compare(expected.as_bytes(), signature.as_bytes())
    }

    // ---- Status mapping ----

    pub fn map_payment_status(raw: &str) -> PaymentStatus {
        match raw.to_lowercase().as_str() {
            "pending" | "created" | "new" => PaymentStatus::Pending,
            "completed" | "paid" | "success" => PaymentStatus::Completed,
            "failed" | "rejected" | "error" => PaymentStatus::Failed,
            "refunded" | "canceled" => PaymentStatus::Refunded,
            "expired" | "timeout" => PaymentStatus::Expired,
            _ => PaymentStatus::Pending,
        }
    }

    // =========================================================================
    // SUBSCRIPTION API — рекуррентные автосписания
    // =========================================================================

    /// Create a new subscription (recurring billing)
    pub async fn create_subscription(
        &self,
        req: &CreateSubscriptionRequest,
    ) -> Result<SubscriptionResponse> {
        let json = serde_json::to_string(req)?;
        let resp = self.http.post("/subscriptions", &json).await?;
        let sub: SubscriptionResponse = serde_json::from_str(&resp)?;

        if let Some(err) = &sub.error_code {
            bail!("Tochka subscription error {}: {}",
                err, sub.error_description.as_deref().unwrap_or(""));
        }
        Ok(sub)
    }

    /// Get subscription status
    pub async fn get_subscription(&self, subscription_id: &str) -> Result<SubscriptionResponse> {
        let path = format!("/subscriptions/{}", subscription_id);
        let resp = self.http.get(&path).await?;
        Ok(serde_json::from_str(&resp)?)
    }

    /// Get subscription by customer ID (our account_id)
    pub async fn get_subscription_by_customer(&self, customer_id: &str) -> Result<SubscriptionResponse> {
        let path = format!("/subscriptions?customer={}", customer_id);
        let resp = self.http.get(&path).await?;
        Ok(serde_json::from_str(&resp)?)
    }

    /// Pause a subscription
    pub async fn pause_subscription(&self, subscription_id: &str) -> Result<SubscriptionResponse> {
        let path = format!("/subscriptions/{}/pause", subscription_id);
        let resp = self.http.post(&path, "").await?;
        Ok(serde_json::from_str(&resp)?)
    }

    /// Resume a paused subscription
    pub async fn resume_subscription(&self, subscription_id: &str) -> Result<SubscriptionResponse> {
        let path = format!("/subscriptions/{}/resume", subscription_id);
        let resp = self.http.post(&path, "").await?;
        Ok(serde_json::from_str(&resp)?)
    }

    /// Cancel a subscription (no more auto-billing)
    pub async fn cancel_subscription(&self, subscription_id: &str) -> Result<SubscriptionResponse> {
        let path = format!("/subscriptions/{}", subscription_id);
        let resp = self.http.delete(&path).await?;
        Ok(serde_json::from_str(&resp)?)
    }

    /// Change subscription plan or period
    pub async fn change_subscription(
        &self,
        subscription_id: &str,
        new_plan_id: Option<&str>,
        new_period: Option<BillingPeriod>,
        new_amount: Option<u64>,
    ) -> Result<SubscriptionResponse> {
        #[derive(Serialize)]
        struct ChangeReq {
            #[serde(skip_serializing_if = "Option::is_none")]
            plan_id: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            period: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            amount: Option<u64>,
        }

        let body = ChangeReq {
            plan_id: new_plan_id.map(|s| s.to_string()),
            period: new_period.map(|p| p.as_str().to_string()),
            amount: new_amount,
        };
        let json = serde_json::to_string(&body)?;
        let path = format!("/subscriptions/{}", subscription_id);
        let resp = self.http.post(&path, &json).await?;
        Ok(serde_json::from_str(&resp)?)
    }

    /// Parse subscription callback webhook
    pub fn parse_subscription_callback(body: &str) -> Result<SubscriptionCallback> {
        Ok(serde_json::from_str(body)?)
    }

    // =========================================================================
    // ACQUIRING API — разовые платежи (доп.услуги, top-up)
    // =========================================================================

    /// Create SBP payment (one-time)
    pub async fn create_sbp_payment(
        &self,
        invoice_id: &str,
        amount_kopecks: u64,
        description: &str,
    ) -> Result<SbpPaymentResponse> {
        let body = SbpPaymentRequest {
            amount: amount_kopecks,
            order: invoice_id.to_string(),
            description: description.to_string(),
            payment_type: self.config.payment_type_id.clone(),
            customer_phone: None,
        };
        let json = serde_json::to_string(&body)?;
        let resp = self.http.post("/acquiring/sbp", &json).await?;
        let payment: SbpPaymentResponse = serde_json::from_str(&resp)?;

        if let Some(err) = &payment.error_code {
            bail!("Tochka SBP error {}: {}", err, payment.error_description.as_deref().unwrap_or(""));
        }
        Ok(payment)
    }

    /// Create card payment (one-time, hosted checkout)
    pub async fn create_card_payment(
        &self,
        invoice_id: &str,
        amount_kopecks: u64,
        description: &str,
    ) -> Result<CardPaymentResponse> {
        let body = CardPaymentRequest {
            amount: amount_kopecks,
            order: invoice_id.to_string(),
            description: description.to_string(),
            language: "ru".to_string(),
            return_url: Some(self.config.success_url.clone()),
            fail_url: Some(self.config.fail_url.clone()),
        };
        let json = serde_json::to_string(&body)?;
        let resp = self.http.post("/acquiring/payment", &json).await?;
        let payment: CardPaymentResponse = serde_json::from_str(&resp)?;

        if let Some(err) = &payment.error_code {
            bail!("Tochka card error {}: {}", err, payment.error_description.as_deref().unwrap_or(""));
        }
        Ok(payment)
    }

    /// Get payment status
    pub async fn get_payment_status(&self, payment_id: &str) -> Result<PaymentStatusResponse> {
        let path = format!("/payments/{}", payment_id);
        let resp = self.http.get(&path).await?;
        Ok(serde_json::from_str(&resp)?)
    }

    /// Refund a payment (full or partial)
    pub async fn refund(&self, payment_id: &str, amount_kopecks: u64) -> Result<RefundResponse> {
        let body = RefundRequest {
            payment_id: payment_id.to_string(),
            amount: amount_kopecks,
        };
        let json = serde_json::to_string(&body)?;
        let resp = self.http.post("/payments/refund", &json).await?;
        let refund: RefundResponse = serde_json::from_str(&resp)?;

        if let Some(err) = &refund.error_code {
            bail!("Tochka refund error {}: {}", err, refund.error_description.as_deref().unwrap_or(""));
        }
        Ok(refund)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    struct MockHttp {
        responses: std::collections::HashMap<String, String>,
    }
    impl MockHttp {
        fn new() -> Self { Self { responses: std::collections::HashMap::new() } }
        fn with(mut self, path: &str, body: &str) -> Self {
            self.responses.insert(path.to_string(), body.to_string()); self
        }
    }

    #[async_trait::async_trait]
    impl TochkaHttp for MockHttp {
        async fn post(&self, path: &str, _body: &str) -> Result<String> {
            self.responses.get(path).cloned()
                .ok_or_else(|| anyhow::anyhow!("No mock: {}", path))
        }
        async fn get(&self, path: &str) -> Result<String> {
            self.responses.get(path).cloned()
                .ok_or_else(|| anyhow::anyhow!("No mock: {}", path))
        }
        async fn delete(&self, path: &str) -> Result<String> {
            self.responses.get(path).cloned()
                .ok_or_else(|| anyhow::anyhow!("No mock: {}", path))
        }
    }

    fn cfg() -> SbpConfig {
        SbpConfig {
            terminal_key: "test".into(),
            secret_key: "test_secret_for_hmac".into(),
            payment_type_id: "SBP".into(),
            callback_url: "https://flowlink.flow-masters.ru/api/billing/callback".into(),
            success_url: "https://flowlink.flow-masters.ru/billing/success".into(),
            fail_url: "https://flowlink.flow-masters.ru/billing/fail".into(),
        }
    }

    fn client() -> TochkaClient {
        let mock = MockHttp::new()
            .with("/subscriptions", r#"{
                "subscription_id": "sub_abc123",
                "customer_id": "acc-1",
                "plan_id": "pro",
                "status": "active",
                "period": "month",
                "amount": 29990,
                "next_billing_date": "2026-05-08T22:00:00Z",
                "current_period_start": "2026-04-08T22:00:00Z",
                "current_period_end": "2026-05-08T22:00:00Z"
            }"#)
            .with("/subscriptions/sub_abc123", r#"{
                "subscription_id": "sub_abc123",
                "customer_id": "acc-1",
                "plan_id": "pro",
                "status": "paused",
                "period": "month",
                "amount": 29990
            }"#)
            .with("/subscriptions/sub_abc123/pause", r#"{
                "subscription_id": "sub_abc123",
                "customer_id": "acc-1",
                "plan_id": "pro",
                "status": "paused",
                "period": "month",
                "amount": 29990
            }"#)
            .with("/subscriptions/sub_abc123/resume", r#"{
                "subscription_id": "sub_abc123",
                "customer_id": "acc-1",
                "plan_id": "pro",
                "status": "active",
                "period": "month",
                "amount": 29990
            }"#)
            .with("/acquiring/sbp", r#"{
                "payment_id": "pay_123",
                "order": "INV-0001",
                "status": "created",
                "payment_url": "https://pay.tochka.com/sbp/abc123",
                "qr_code": "https://qr.nspk.ru/AS1000..."
            }"#)
            .with("/acquiring/payment", r#"{
                "payment_id": "card_789",
                "order": "INV-0002",
                "status": "created",
                "payment_url": "https://pay.tochka.com/checkout/card_789"
            }"#)
            .with("/payments/pay_123", r#"{
                "payment_id": "pay_123",
                "order": "INV-0001",
                "status": "completed",
                "amount": 29990
            }"#);
        TochkaClient::with_http(cfg(), Box::new(mock))
    }

    // ---- BillingPeriod tests ----

    #[test]
    fn test_billing_period_days() {
        assert_eq!(BillingPeriod::Month.days(), 30);
        assert_eq!(BillingPeriod::Quarter.days(), 90);
        assert_eq!(BillingPeriod::Year.days(), 365);
        assert_eq!(BillingPeriod::Days(14).days(), 14);
    }

    #[test]
    fn test_billing_period_multiplier() {
        assert!((BillingPeriod::Quarter.price_multiplier() - 2.7).abs() < 0.01);
        assert!((BillingPeriod::Year.price_multiplier() - 10.0).abs() < 0.01);
        assert_eq!(BillingPeriod::Month.price_multiplier(), 1.0);
    }

    #[test]
    fn test_billing_period_parse() {
        assert_eq!(BillingPeriod::from_str_opt("month"), Some(BillingPeriod::Month));
        assert_eq!(BillingPeriod::from_str_opt("quarterly"), Some(BillingPeriod::Quarter));
        assert_eq!(BillingPeriod::from_str_opt("annual"), Some(BillingPeriod::Year));
        assert_eq!(BillingPeriod::from_str_opt("weekly"), None);
    }

    // ---- SubscriptionStatus tests ----

    #[test]
    fn test_subscription_status() {
        assert!(SubscriptionStatus::Active.is_serving());
        assert!(SubscriptionStatus::Trial.is_serving());
        assert!(SubscriptionStatus::PastDue.is_serving());
        assert!(!SubscriptionStatus::Paused.is_serving());
        assert!(!SubscriptionStatus::Cancelled.is_serving());
    }

    // ---- Signing ----

    #[test]
    fn test_sign_deterministic() {
        let c = client();
        let s1 = c.sign("hello");
        let s2 = c.sign("hello");
        assert_eq!(s1, s2);
        assert_eq!(s1.len(), 64);
    }

    #[test]
    fn test_verify_valid() {
        let c = client();
        assert!(c.verify_signature("test", &c.sign("test")));
    }

    #[test]
    fn test_verify_invalid() {
        let c = client();
        assert!(!c.verify_signature("test", "wrong"));
    }

    // ---- Subscription API ----

    #[tokio::test]
    async fn test_create_subscription() {
        let c = client();
        let req = CreateSubscriptionRequest {
            customer_id: "acc-1".into(),
            plan_id: "pro".into(),
            period: BillingPeriod::Month,
            amount: 29990,
            payment_method: SubscriptionPaymentMethod::Sbp { phone: "+79001234567".into() },
            description: "FlowLink Pro".into(),
            start_date: None,
            trial_days: 0,
        };
        let sub = c.create_subscription(&req).await.unwrap();
        assert_eq!(sub.subscription_id, "sub_abc123");
        assert_eq!(sub.status, "active");
    }

    #[tokio::test]
    async fn test_pause_subscription() {
        let c = client();
        let sub = c.pause_subscription("sub_abc123").await.unwrap();
        assert_eq!(sub.status, "paused");
    }

    #[tokio::test]
    async fn test_resume_subscription() {
        let c = client();
        let sub = c.resume_subscription("sub_abc123").await.unwrap();
        assert_eq!(sub.status, "active");
    }

    // ---- Acquiring API ----

    #[tokio::test]
    async fn test_create_sbp_payment() {
        let c = client();
        let p = c.create_sbp_payment("INV-0001", 29990, "Pro подписка").await.unwrap();
        assert_eq!(p.payment_id, "pay_123");
        assert!(p.payment_url.is_some());
    }

    #[tokio::test]
    async fn test_create_card_payment() {
        let c = client();
        let p = c.create_card_payment("INV-0002", 29990, "Pro подписка").await.unwrap();
        assert_eq!(p.payment_id, "card_789");
        assert!(p.payment_url.is_some());
    }

    #[tokio::test]
    async fn test_get_payment_status() {
        let c = client();
        let s = c.get_payment_status("pay_123").await.unwrap();
        assert_eq!(s.status, "completed");
    }

    // ---- Callback parsing ----

    #[test]
    fn test_parse_subscription_callback() {
        let body = r#"{
            "subscription_id": "sub_abc123",
            "event": "renewed",
            "status": "active",
            "payment_id": "pay_new",
            "amount": 29990,
            "timestamp": "2026-05-08T22:00:00Z",
            "signature": "abc"
        }"#;
        let cb = TochkaClient::parse_subscription_callback(body).unwrap();
        assert_eq!(cb.event, "renewed");
        assert_eq!(cb.amount, Some(29990));
    }

    // ---- Status mapping ----

    #[test]
    fn test_map_payment_status() {
        assert_eq!(TochkaClient::map_payment_status("completed"), PaymentStatus::Completed);
        assert_eq!(TochkaClient::map_payment_status("failed"), PaymentStatus::Failed);
        assert_eq!(TochkaClient::map_payment_status("expired"), PaymentStatus::Expired);
    }
}
