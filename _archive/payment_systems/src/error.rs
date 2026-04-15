use thiserror::Error;

#[derive(Debug, Error)]
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

    #[error("Webhook error: {0}")]
    WebhookError(String),
}