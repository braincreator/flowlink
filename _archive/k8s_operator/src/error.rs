use thiserror::Error;

#[derive(Debug, Error)]
pub enum K8sOperatorError {
    #[error("Kubernetes client error: {0}")]
    KubernetesError(String),

    #[error("Resource not found: {0}")]
    ResourceNotFound(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    #[error("Watch error: {0}")]
    WatchError(String),

    #[error("Reconciliation error: {0}")]
    ReconciliationError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Leader election error: {0}")]
    LeaderElectionError(String),
}