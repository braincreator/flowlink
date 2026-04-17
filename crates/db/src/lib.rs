//! FlowLink Database Layer
//!
//! PostgreSQL-backed persistence via Supabase for billing, audit, registry.
//!
//! # Tables
//!
//! ```sql
//! plans            -- billing plans (dynamic pricing)
//! accounts         -- account billing state
//! usage_daily      -- daily usage counters
//! invoices         -- invoices
//! invoice_items    -- invoice line items
//! audit_log        -- audit entries
//! agents           -- registered agents
//! devices          -- paired devices
//! ```

pub mod accounts;
pub mod audit;
pub mod email_verification;
pub mod invoices;
pub mod migrations;
pub mod notification_channels;
pub mod orders;
pub mod plans;
pub mod pool;
pub mod subscriptions;
pub mod orgs;
pub mod usage;

pub use pool::DbPool;
