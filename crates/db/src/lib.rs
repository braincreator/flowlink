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

pub mod pool;
pub mod accounts;
pub mod usage;
pub mod invoices;
pub mod audit;
pub mod subscriptions;
pub mod orders;
pub mod plans;
pub mod migrations;

pub use pool::DbPool;
