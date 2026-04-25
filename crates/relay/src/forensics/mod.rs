//! Forensics module — incident timeline, reconstruction, and audit reports.
//!
//! Builds on existing audit_log, command_history, infra_map, and shield data
//! to provide incident reconstruction and compliance-ready reports.

pub mod timeline;
pub mod report;
pub mod snapshot_context;

pub use timeline::IncidentTimeline;
pub use report::ForensicReport;
pub use snapshot_context::ContextSnapshot;
