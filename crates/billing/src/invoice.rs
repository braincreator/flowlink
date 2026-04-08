//! Invoice generation and storage
//!
//! Generates invoices for plan upgrades and usage overages.
//! Supports JSON format (PDF via external renderer).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use uuid::Uuid;

use crate::plans::Plan;
use crate::payment::PaymentMethod;

/// Invoice status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvoiceStatus {
    /// Invoice created, awaiting payment
    Pending,
    /// Payment received
    Paid,
    /// Payment failed
    Failed,
    /// Invoice cancelled
    Cancelled,
    /// Partially refunded
    Refunded,
    /// Invoice overdue
    Overdue,
}

/// Invoice line item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceLineItem {
    /// Line description
    pub description: String,
    /// Quantity
    pub quantity: u64,
    /// Unit price in kopecks
    pub unit_price_kopecks: u64,
    /// Total in kopecks
    pub total_kopecks: u64,
}

impl InvoiceLineItem {
    pub fn new(description: &str, quantity: u64, unit_price_kopecks: u64) -> Self {
        Self {
            description: description.to_string(),
            quantity,
            unit_price_kopecks,
            total_kopecks: quantity * unit_price_kopecks,
        }
    }
}

/// An invoice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invoice {
    /// Unique invoice ID
    pub id: String,
    /// Account ID
    pub account_id: String,
    /// Invoice number (human-readable, e.g. "INV-2026-0001")
    pub number: String,
    /// Status
    pub status: InvoiceStatus,
    /// Line items
    pub items: Vec<InvoiceLineItem>,
    /// Subtotal in kopecks
    pub subtotal_kopecks: u64,
    /// Tax (NDS 20%) in kopecks
    pub tax_kopecks: u64,
    /// Total in kopecks
    pub total_kopecks: u64,
    /// Currency
    pub currency: String,
    /// Payment method used
    pub payment_method: Option<PaymentMethod>,
    /// Created at
    pub created_at: DateTime<Utc>,
    /// Paid at
    pub paid_at: Option<DateTime<Utc>>,
    /// Due date
    pub due_at: DateTime<Utc>,
    /// Notes
    pub notes: Option<String>,
}

impl Invoice {
    /// Create a new invoice
    pub fn new(account_id: &str, items: Vec<InvoiceLineItem>) -> Self {
        let subtotal: u64 = items.iter().map(|i| i.total_kopecks).sum();
        let tax = (subtotal as f64 * 0.20).round() as u64; // 20% NDS
        let total = subtotal + tax;
        let now = Utc::now();

        Self {
            id: Uuid::new_v4().to_string(),
            account_id: account_id.to_string(),
            number: format!("INV-{}", now.format("%Y%m%d-%H%M%S")),
            status: InvoiceStatus::Pending,
            items,
            subtotal_kopecks: subtotal,
            tax_kopecks: tax,
            total_kopecks: total,
            currency: "RUB".to_string(),
            payment_method: None,
            created_at: now,
            paid_at: None,
            due_at: now + chrono::Duration::days(7),
            notes: None,
        }
    }

    /// Create a plan subscription invoice
    pub fn for_plan(account_id: &str, plan: &Plan) -> Self {
        let items = vec![
            InvoiceLineItem::new(
                &format!("Подписка FlowLink {} (1 месяц)", plan.name),
                1,
                plan.price_kopecks,
            ),
        ];
        Self::new(account_id, items)
    }

    /// Create an overage invoice
    pub fn for_overage(
        account_id: &str,
        extra_requests: u64,
        extra_tokens: u64,
        request_price_kopecks: u64,
        token_price_kopecks: u64,
    ) -> Self {
        let mut items = Vec::new();
        if extra_requests > 0 {
            items.push(InvoiceLineItem::new(
                "Дополнительные API запросы",
                extra_requests,
                request_price_kopecks,
            ));
        }
        if extra_tokens > 0 {
            items.push(InvoiceLineItem::new(
                "Дополнительные токены (за 1K)",
                extra_tokens.div_ceil(1000), // round up to 1K
                token_price_kopecks,
            ));
        }
        let mut invoice = Self::new(account_id, items);
        invoice.notes = Some("Оплата перерасхода лимитов плана".to_string());
        invoice
    }

    /// Mark as paid
    pub fn mark_paid(&mut self, method: PaymentMethod) {
        self.status = InvoiceStatus::Paid;
        self.payment_method = Some(method);
        self.paid_at = Some(Utc::now());
    }

    /// Mark as failed
    pub fn mark_failed(&mut self) {
        self.status = InvoiceStatus::Failed;
    }

    /// Mark as cancelled
    pub fn mark_cancelled(&mut self) {
        self.status = InvoiceStatus::Cancelled;
    }

    /// Check if overdue
    pub fn is_overdue(&self) -> bool {
        self.status == InvoiceStatus::Pending && Utc::now() > self.due_at
    }

    /// Format total in RUB
    pub fn format_total(&self) -> String {
        Plan::format_price(self.total_kopecks)
    }

    /// Format subtotal in RUB
    pub fn format_subtotal(&self) -> String {
        Plan::format_price(self.subtotal_kopecks)
    }

    /// Format tax in RUB
    pub fn format_tax(&self) -> String {
        Plan::format_price(self.tax_kopecks)
    }
}

/// Invoice store — in-memory storage for invoices
pub struct InvoiceStore {
    /// By invoice ID
    by_id: RwLock<HashMap<String, Invoice>>,
    /// By account ID
    by_account: RwLock<HashMap<String, Vec<String>>>,
    /// Counter for invoice numbers
    counter: RwLock<u64>,
}

impl InvoiceStore {
    pub fn new() -> Self {
        Self {
            by_id: RwLock::new(HashMap::new()),
            by_account: RwLock::new(HashMap::new()),
            counter: RwLock::new(0),
        }
    }

    /// Create and store a new invoice
    pub fn create(&self, mut invoice: Invoice) -> Invoice {
        let mut counter = self.counter.write().unwrap();
        *counter += 1;
        invoice.number = format!("INV-{:04}", *counter);

        let id = invoice.id.clone();
        let account_id = invoice.account_id.clone();

        self.by_id.write().unwrap().insert(id.clone(), invoice.clone());
        self.by_account
            .write().unwrap()
            .entry(account_id)
            .or_default()
            .push(id);

        invoice
    }

    /// Get invoice by ID
    pub fn get(&self, id: &str) -> Option<Invoice> {
        self.by_id.read().unwrap().get(id).cloned()
    }

    /// List invoices for an account
    pub fn list_for_account(&self, account_id: &str) -> Vec<Invoice> {
        let by_account = self.by_account.read().unwrap();
        let by_id = self.by_id.read().unwrap();

        by_account
            .get(account_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| by_id.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// List all pending invoices
    pub fn list_pending(&self) -> Vec<Invoice> {
        self.by_id.read().unwrap()
            .values()
            .filter(|i| i.status == InvoiceStatus::Pending)
            .cloned()
            .collect()
    }

    /// List all overdue invoices
    pub fn list_overdue(&self) -> Vec<Invoice> {
        self.by_id.read().unwrap()
            .values()
            .filter(|i| i.is_overdue())
            .cloned()
            .collect()
    }

    /// Update an existing invoice
    pub fn update(&self, invoice: Invoice) {
        self.by_id.write().unwrap().insert(invoice.id.clone(), invoice);
    }

    /// Total revenue (sum of paid invoices) in kopecks
    pub fn total_revenue(&self) -> u64 {
        self.by_id.read().unwrap()
            .values()
            .filter(|i| i.status == InvoiceStatus::Paid)
            .map(|i| i.total_kopecks)
            .sum()
    }

    /// Revenue for an account in kopecks
    pub fn account_revenue(&self, account_id: &str) -> u64 {
        self.by_id.read().unwrap()
            .values()
            .filter(|i| i.account_id == account_id && i.status == InvoiceStatus::Paid)
            .map(|i| i.total_kopecks)
            .sum()
    }
}

impl Default for InvoiceStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plans::Plan;

    fn make_store() -> InvoiceStore {
        InvoiceStore::new()
    }

    #[test]
    fn test_create_invoice() {
        let store = make_store();
        let items = vec![InvoiceLineItem::new("Test", 2, 1000)];
        let invoice = store.create(Invoice::new("acc-1", items));

        assert_eq!(invoice.status, InvoiceStatus::Pending);
        assert_eq!(invoice.subtotal_kopecks, 2000);
        assert_eq!(invoice.tax_kopecks, 400); // 20%
        assert_eq!(invoice.total_kopecks, 2400);
        assert_eq!(invoice.number, "INV-0001");
    }

    #[test]
    fn test_invoice_for_plan() {
        let plan = Plan::pro();
        let store = make_store();
        let invoice = store.create(Invoice::for_plan("acc-1", &plan));

        assert_eq!(invoice.items.len(), 1);
        assert_eq!(invoice.subtotal_kopecks, 29_990);
        assert!(invoice.total_kopecks > 29_990); // with tax
    }

    #[test]
    fn test_mark_paid() {
        let store = make_store();
        let mut invoice = store.create(Invoice::for_plan("acc-1", &Plan::pro()));
        invoice.mark_paid(PaymentMethod::Sbp);
        store.update(invoice.clone());

        assert_eq!(invoice.status, InvoiceStatus::Paid);
        assert!(invoice.paid_at.is_some());
        assert_eq!(store.total_revenue(), invoice.total_kopecks);
    }

    #[test]
    fn test_list_for_account() {
        let store = make_store();
        store.create(Invoice::for_plan("acc-1", &Plan::pro()));
        store.create(Invoice::for_plan("acc-1", &Plan::pro()));
        store.create(Invoice::for_plan("acc-2", &Plan::free()));

        assert_eq!(store.list_for_account("acc-1").len(), 2);
        assert_eq!(store.list_for_account("acc-2").len(), 1);
        assert_eq!(store.list_for_account("acc-3").len(), 0);
    }

    #[test]
    fn test_overage_invoice() {
        let store = make_store();
        let invoice = store.create(Invoice::for_overage(
            "acc-1", 500, 1_500_000, 10, 50,
        ));

        assert_eq!(invoice.items.len(), 2);
        assert!(invoice.notes.unwrap().contains("перерасход"));
    }

    #[test]
    fn test_format_price() {
        let plan = Plan::free();
        let invoice = Invoice::for_plan("acc-1", &plan);
        assert_eq!(invoice.format_total(), "0.00 ₽");
    }

    #[test]
    fn test_total_revenue() {
        let store = make_store();

        let mut inv1 = store.create(Invoice::for_plan("acc-1", &Plan::pro()));
        inv1.mark_paid(PaymentMethod::Card);
        store.update(inv1);

        let _inv2 = store.create(Invoice::for_plan("acc-2", &Plan::pro()));
        // inv2 is pending, not counted

        assert_eq!(store.total_revenue(), 35_988); // 29990 + 20% NDS
    }

    #[test]
    fn test_invoice_number_sequential() {
        let store = make_store();
        let inv1 = store.create(Invoice::new("a", vec![]));
        let inv2 = store.create(Invoice::new("a", vec![]));
        assert_eq!(inv1.number, "INV-0001");
        assert_eq!(inv2.number, "INV-0002");
    }
}
