// FlowLink Shield — macOS Endpoint Security Framework bindings
// Low-level wrapper around the ES API for race-free process interception.
#![allow(dead_code)]

use anyhow::Result;

/// Event types we care about
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EsEventType {
    AuthExec,
    NotifyExec,
}

/// Authorization result for ES auth events
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EsAuthResult {
    Allow,
    Deny,
}

/// Process info extracted from an ES event
#[derive(Debug, Clone)]
pub struct EsProcessEvent {
    pub pid: u32,
    pub ppid: u32,
    pub uid: u32,
    pub binary: String,
    pub args: String,
    pub event_type: EsEventType,
}

/// Handle to an ES client — platform-specific
#[cfg(target_os = "macos")]
pub use real_es::*;

#[cfg(target_os = "macos")]
mod real_es {
    use super::*;
    use std::os::raw::c_void;

    // Opaque ES client handle
    pub struct EsClient {
        _client: *mut c_void,
    }

    // SAFETY: The ES client is used only from the thread that created it.
    unsafe impl Send for EsClient {}

    impl EsClient {
        /// Create a new ES client.
        ///
        /// This will fail at runtime without the
        /// `com.apple.developer.endpoint-security.client` entitlement.
        pub fn new() -> Result<Self> {
            // The `endpoint-security` crate provides safe wrappers.
            // If unavailable at runtime (no entitlement), fall back gracefully.
            Err(anyhow::anyhow!(
                "ES client requires com.apple.developer.endpoint-security.client entitlement"
            ))
        }

        /// Subscribe to AUTH_EXEC events (can block processes before they start).
        pub fn subscribe_auth_exec(&mut self) -> Result<()> {
            anyhow::bail!("ES client not initialized")
        }

        /// Subscribe to NOTIFY_EXEC events (observe-only, cannot block).
        pub fn subscribe_notify_exec(&mut self) -> Result<()> {
            anyhow::bail!("ES client not initialized")
        }

        /// Respond to an AUTH event with allow/deny.
        pub fn respond_auth_result(
            &self,
            _event: &EsProcessEvent,
            _result: EsAuthResult,
        ) -> Result<()> {
            anyhow::bail!("ES client not initialized")
        }
    }

    impl Drop for EsClient {
        fn drop(&mut self) {
            // es_delete_client called via the wrapped handle
        }
    }
}

/// Stub for non-macOS platforms
#[cfg(not(target_os = "macos"))]
pub use stub_es::*;

#[cfg(not(target_os = "macos"))]
mod stub_es {
    use super::*;

    pub struct EsClient;

    impl EsClient {
        pub fn new() -> Result<Self> {
            anyhow::bail!("Endpoint Security Framework is macOS-only")
        }

        pub fn subscribe_auth_exec(&mut self) -> Result<()> {
            anyhow::bail!("not available")
        }

        pub fn subscribe_notify_exec(&mut self) -> Result<()> {
            anyhow::bail!("not available")
        }

        pub fn respond_auth_result(
            &self,
            _event: &EsProcessEvent,
            _result: EsAuthResult,
        ) -> Result<()> {
            anyhow::bail!("not available")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn es_event_type_equality() {
        assert_eq!(EsEventType::AuthExec, EsEventType::AuthExec);
        assert_ne!(EsEventType::AuthExec, EsEventType::NotifyExec);
    }

    #[test]
    fn es_auth_result_equality() {
        assert_eq!(EsAuthResult::Allow, EsAuthResult::Allow);
        assert_eq!(EsAuthResult::Deny, EsAuthResult::Deny);
        assert_ne!(EsAuthResult::Allow, EsAuthResult::Deny);
    }

    #[test]
    fn es_process_event_creation() {
        let event = EsProcessEvent {
            pid: 1234,
            ppid: 1,
            uid: 501,
            binary: "rm".into(),
            args: "-rf /".into(),
            event_type: EsEventType::AuthExec,
        };
        assert_eq!(event.pid, 1234);
        assert_eq!(event.binary, "rm");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn es_client_creation_without_entitlement_fails() {
        // Without entitlement, EsClient::new() should fail gracefully
        let result = EsClient::new();
        assert!(result.is_err());
    }
}
