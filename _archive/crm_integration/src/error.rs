use thiserror::Error;

#[derive(Debug, Error)]
pub enum CRMIntegrationError {
    #[error("Webhook signature verification failed: {0}")]
    WebhookVerificationFailed(String),

    #[error("Unsupported CRM provider: {0}")]
    UnsupportedProvider(String),

    #[error("Invalid webhook payload: {0}")]
    InvalidPayload(String),

    #[error("No handler registered for provider: {0}")]
    NoHandlerRegistered(String),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("API request failed: {0}")]
    ApiRequestFailed(String),

    #[error("Failed to sync data: {0}")]
    SyncFailed(String),

    #[error("Failed to create entity mapping: {0}")]
    EntityMappingFailed(String),

    #[error("Failed to execute workflow: {0}")]
    WorkflowExecutionFailed(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),
}