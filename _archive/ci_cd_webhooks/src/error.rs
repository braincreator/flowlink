use thiserror::Error;

#[derive(Debug, Error)]
pub enum CIWebhookError {
    #[error("Webhook signature verification failed: {0}")]
    SignatureVerificationFailed(String),

    #[error("Unsupported provider: {0}")]
    UnsupportedProvider(String),

    #[error("Invalid webhook payload: {0}")]
    InvalidPayload(String),

    #[error("No handler registered for provider: {0}")]
    NoHandlerRegistered(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Deployment failed: {0}")]
    DeploymentFailed(String),

    #[error("Failed to process event: {0}")]
    EventProcessingFailed(String),

    #[error("Failed to send notification: {0}")]
    NotificationFailed(String),
}