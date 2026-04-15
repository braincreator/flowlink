//! Pipeline module for command processing and protection
//!
//! This module contains the shield pipeline components:
//! - `literal_checker`: Rejects shell expansion in destructive commands
//! - `tempo`: Circuit breaker and rate limiting controller
//! - `classifier`: Tiered action classification
//! - `feedback`: Structured denial messages
//! - `orchestrator`: L3 pipeline — wires all components together

pub mod classifier;
pub mod feedback;
pub mod literal_checker;
pub mod orchestrator;
pub mod tempo;

pub use classifier::ActionClassifier;
pub use feedback::DenialFeedbackBuilder;
pub use literal_checker::LiteralChecker;
pub use orchestrator::PipelineAction;
pub use orchestrator::PipelineOrchestrator;
pub use orchestrator::PipelineResult;
pub use tempo::TempoController;
