use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

// Payment provider models
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PaymentProviderConfig {
    pub provider_type: String,
    pub api_key: String,
    pub api_secret: String,
    pub sandbox: bool,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct YooKassaConfig {
    pub shop_id: String,
    pub secret_key: String,
    pub is_sandbox: bool,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct QiwiConfig {
    pub wallet_id: String,
    pub api_key: String,
    pub is_sandbox: bool,
}

// Payment invoice models
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PaymentInvoice {
    pub id: String,
    pub provider: String,
    pub customer_id: String,
    pub amount: Option<f64>,
    pub currency: String,
    pub description: String,
    pub status: PaymentStatus,
    pub payment_id: Option<String>,
    pub payment_url: Option<String>,
    pub metadata: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub paid_at: Option<DateTime<Utc>>,
    pub failed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum PaymentStatus {
    Pending,
    Paid,
    Failed,
    Cancelled,
    Refunded,
    Processing,
}

impl PaymentStatus {
    pub fn as_str(&self) -> &str {
        match self {
            PaymentStatus::Pending => "pending",
            PaymentStatus::Paid => "paid",
            PaymentStatus::Failed => "failed",
            PaymentStatus::Cancelled => "cancelled",
            PaymentStatus::Refunded => "refunded",
            PaymentStatus::Processing => "processing",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self,
            PaymentStatus::Paid |
            PaymentStatus::Failed |
            PaymentStatus::Cancelled |
            PaymentStatus::Refunded
        )
    }
}

// Subscription models
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Subscription {
    pub id: String,
    pub provider: String,
    pub customer_id: String,
    pub plan_id: String,
    pub amount: f64,
    pub currency: String,
    pub period: SubscriptionPeriod,
    pub status: SubscriptionStatus,
    pub current_period_start: DateTime<Utc>,
    pub current_period_end: DateTime<Utc>,
    pub trial_end: Option<DateTime<Utc>>,
    pub auto_renew: bool,
    pub cancel_at_period_end: bool,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum SubscriptionPeriod {
    Monthly,
    Quarterly,
    Annually,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum SubscriptionStatus {
    Active,
    Trial,
    PastDue,
    Cancelled,
    Expired,
}

impl SubscriptionStatus {
    pub fn as_str(&self) -> &str {
        match self {
            SubscriptionStatus::Active => "active",
            SubscriptionStatus::Trial => "trial",
            SubscriptionStatus::PastDue => "past_due",
            SubscriptionStatus::Cancelled => "cancelled",
            SubscriptionStatus::Expired => "expired",
        }
    }
}

// Refund models
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct RefundRequest {
    pub amount: Option<f64>,
    pub reason: Option<String>,
    pub description: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Refund {
    pub id: String,
    pub invoice_id: String,
    pub provider: String,
    pub amount: Option<f64>,
    pub status: RefundStatus,
    pub reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum RefundStatus {
    Pending,
    Processed,
    Failed,
}

// Payment result
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PaymentResult {
    pub success: bool,
    pub payment_id: String,
    pub invoice_id: Option<String>,
    pub amount: f64,
    pub currency: String,
    pub status: PaymentStatus,
    pub message: String,
}

// Webhook data models
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct WebhookData {
    pub event_type: String,
    pub id: String,
    pub date: DateTime<Utc>,
    pub data: WebhookDataContent,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct WebhookDataContent {
    #[serde(rename = "type")]
    pub data_type: String,
    pub object: serde_json::Value,
}

// YooKassa specific models
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct YooKassaInvoice {
    pub id: String,
    pub amount: YooKassaAmount,
    pub payment_method: Option<YooKassaPaymentMethod>,
    pub status: YooKassaStatus,
    pub pay_url: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct YooKassaAmount {
    pub value: String,
    pub currency: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct YooKassaPaymentMethod {
    pub type_: YooKassaPaymentType,
    pub saved: bool,
    pub token: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum YooKassaPaymentType {
    Card,
    BankCard,
    ApplePay,
    GooglePay,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum YooKassaStatus {
    Canceled,
    Pending,
    WaitingForCapture,
    Successful,
}

// Qiwi specific models
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct QiwiInvoice {
    pub bill_id: String,
    pub status: QiwiStatus,
    pub sum: f64,
    pub currency: String,
    pub expirationDateTime: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum QiwiStatus {
    BillWaiting,
    BillPaid,
    BillCanceled,
}

// Payment provider trait
#[async_trait::async_trait]
pub trait PaymentProvider: Send + Sync {
    async fn create_invoice(
        &self,
        config: &YooKassaConfig,
        amount: f64,
        currency: &str,
        description: &str,
        customer_id: &str,
        metadata: HashMap<String, String>,
    ) -> Result<PaymentInvoice>;

    async fn create_subscription(
        &self,
        config: &YooKassaConfig,
        amount: f64,
        period: SubscriptionPeriod,
        customer_id: &str,
        plan_id: &str,
    ) -> Result<Subscription>;

    async fn process_payment(
        &self,
        config: &YooKassaConfig,
        payment_id: &str,
        payment_data: &str,
    ) -> Result<PaymentResult>;

    async fn create_refund(
        &self,
        config: &YooKassaConfig,
        request: &RefundRequest,
    ) -> Result<Refund>;

    async fn verify_webhook_signature(
        &self,
        config: &PaymentProviderConfig,
        signature: &str,
        payload: &str,
    ) -> Result<bool>;

    async fn parse_webhook(&self, payload: &str) -> Result<WebhookData>;

    fn get_provider_type(&self) -> &str;
}

// Notification service
#[async_trait::async_trait]
pub trait NotificationService: Send + Sync {
    async fn send_invoice_notification(&self, invoice: &PaymentInvoice) -> Result<()>;
    async fn send_payment_notification(&self, result: &PaymentResult) -> Result<()>;
    async fn send_subscription_notification(&self, subscription: &Subscription) -> Result<()>;
    async fn send_refund_notification(&self, refund: &Refund) -> Result<()>;
    async fn send_subscription_cancelled_notification(&self, subscription: &Subscription) -> Result<()>;
}

// Error types
#[derive(Debug, thiserror::Error)]
pub enum PaymentError {
    #[error("Unknown provider type: {0}")]
    UnknownProvider(String),

    #[error("Provider not found: {0}")]
    ProviderNotFound(String),

    #[error("Invalid API credentials")]
    InvalidCredentials,

    #[error("Payment failed: {0}")]
    PaymentFailed(String),

    #[error("Invoice not found: {0}")]
    InvoiceNotFound(String),

    #[error("Subscription not found: {0}")]
    SubscriptionNotFound(String),

    #[error("Invalid amount: {0}")]
    InvalidAmount(String),

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),
}

// Standard payment plans
pub const PLAN_BASIC: &str = "basic";
pub const PLAN_PRO: &str = "pro";
pub const PLAN_ENTERPRISE: &str = "enterprise";

pub const PLAN_BASIC_AMOUNT: f64 = 999.0; // 999 RUB
pub const PLAN_PRO_AMOUNT: f64 = 2999.0; // 2999 RUB
pub const PLAN_ENTERPRISE_AMOUNT: f64 = 9999.0; // 9999 RUB