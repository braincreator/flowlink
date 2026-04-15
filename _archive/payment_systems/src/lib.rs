pub mod models;
pub mod yookassa;
pub mod qiwi;
pub mod storage;
pub mod error;
pub mod invoice;
pub mod subscription;
pub mod refund;
pub mod notification;

pub use models::*;
pub use yookassa::*;
pub use qiwi::*;
pub use storage::*;
pub use error::*;
pub use invoice::*;
pub use subscription::*;
pub use refund::*;
pub use notification::*;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

// Main payment manager
pub struct PaymentManager {
    pub providers: Arc<RwLock<HashMap<String, Arc<dyn PaymentProvider + Send + Sync>>>>,
    pub storage: Arc<PaymentStorage>,
    pub notification_service: Arc<NotificationService>,
}

impl PaymentManager {
    pub fn new(
        storage: Arc<PaymentStorage>,
        notification_service: Arc<NotificationService>,
    ) -> Self {
        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
            storage,
            notification_service,
        }
    }

    pub async fn register_provider(&self, provider: PaymentProviderConfig) -> Result<()> {
        let provider_impl = match provider.provider_type.as_str() {
            "yookassa" => Arc::new(YooKassaProvider::new(provider)?),
            "qiwi" => Arc::new(QiwiProvider::new(provider)?),
            _ => {
                return Err(PaymentError::UnknownProvider(provider.provider_type));
            }
        };

        self.providers.write().await.insert(provider.provider_type.clone(), provider_impl);

        log::info!("Registered payment provider: {}", provider.provider_type);
        Ok(())
    }

    pub async fn get_provider(&self, provider_type: &str) -> Result<Arc<dyn PaymentProvider + Send + Sync>> {
        let providers = self.providers.read().await;

        providers.get(provider_type)
            .cloned()
            .ok_or_else(|| PaymentError::ProviderNotFound(provider_type.to_string()))
    }

    pub async fn create_invoice(
        &self,
        provider_type: &str,
        amount: f64,
        currency: &str,
        description: &str,
        customer_id: &str,
        metadata: HashMap<String, String>,
    ) -> Result<PaymentInvoice> {
        let provider = self.get_provider(provider_type).await?;
        let config = self.storage.get_provider_config(provider_type).await?;

        let invoice = provider.create_invoice(
            &config,
            amount,
            currency,
            description,
            customer_id,
            metadata,
        ).await?;

        // Save invoice
        self.storage.save_invoice(&invoice).await?;

        // Send notification
        self.notification_service
            .send_invoice_notification(&invoice)
            .await?;

        Ok(invoice)
    }

    pub async fn create_subscription(
        &self,
        provider_type: &str,
        amount: f64,
        period: SubscriptionPeriod,
        customer_id: &str,
        plan_id: &str,
    ) -> Result<Subscription> {
        let provider = self.get_provider(provider_type).await?;
        let config = self.storage.get_provider_config(provider_type).await?;

        let subscription = provider.create_subscription(
            &config,
            amount,
            period,
            customer_id,
            plan_id,
        ).await?;

        // Save subscription
        self.storage.save_subscription(&subscription).await?;

        // Send notification
        self.notification_service
            .send_subscription_notification(&subscription)
            .await?;

        Ok(subscription)
    }

    pub async fn process_payment(
        &self,
        provider_type: &str,
        payment_id: &str,
        payment_data: &str,
    ) -> Result<PaymentResult> {
        let provider = self.get_provider(provider_type).await?;
        let config = self.storage.get_provider_config(provider_type).await?;

        let result = provider.process_payment(&config, payment_id, payment_data).await?;

        // Update invoice status
        if let Some(invoice_id) = &result.invoice_id {
            if let Ok(invoice) = self.storage.get_invoice(invoice_id).await {
                self.storage.update_invoice_status(invoice_id, PaymentStatus::Paid).await?;
            }
        }

        // Send notification
        self.notification_service
            .send_payment_notification(&result)
            .await?;

        Ok(result)
    }

    pub async fn create_refund(
        &self,
        provider_type: &str,
        refund_request: RefundRequest,
    ) -> Result<Refund> {
        let provider = self.get_provider(provider_type).await?;
        let config = self.storage.get_provider_config(provider_type).await?;

        let refund = provider.create_refund(&config, &refund_request).await?;

        // Update invoice status
        if let Some(invoice_id) = &refund.invoice_id {
            self.storage.update_invoice_status(invoice_id, PaymentStatus::Refunded).await?;
        }

        // Send notification
        self.notification_service
            .send_refund_notification(&refund)
            .await?;

        Ok(refund)
    }

    pub async fn verify_webhook(
        &self,
        provider_type: &str,
        signature: &str,
        payload: &str,
    ) -> Result<bool> {
        let provider = self.get_provider(provider_type).await?;

        let config = self.storage.get_provider_config(provider_type).await?;

        provider.verify_webhook_signature(&config, signature, payload).await
    }

    pub async fn handle_webhook(
        &self,
        provider_type: &str,
        signature: &str,
        payload: &str,
    ) -> Result<WebhookResponse> {
        let provider = self.get_provider(provider_type).await?;

        // Verify signature first
        if !provider.verify_webhook_signature(&self.storage.get_provider_config(provider_type).await?, signature, payload).await? {
            return Ok(WebhookResponse {
                success: false,
                message: "Invalid signature".to_string(),
            });
        }

        // Parse webhook
        let webhook_data = provider.parse_webhook(payload).await?;

        // Process webhook based on event type
        let response = match webhook_data.event_type.as_str() {
            "payment_succeeded" => {
                self.handle_payment_succeeded(&webhook_data).await?
            }
            "payment_failed" => {
                self.handle_payment_failed(&webhook_data).await?
            }
            "invoice_created" => {
                self.handle_invoice_created(&webhook_data).await?
            }
            "subscription_created" => {
                self.handle_subscription_created(&webhook_data).await?
            }
            "subscription_updated" => {
                self.handle_subscription_updated(&webhook_data).await?
            }
            "subscription_cancelled" => {
                self.handle_subscription_cancelled(&webhook_data).await?
            }
            _ => {
                Ok(WebhookResponse {
                    success: true,
                    message: "Unhandled event".to_string(),
                })
            }
        };

        Ok(response)
    }

    pub async fn get_invoices_by_customer(&self, customer_id: &str) -> Result<Vec<PaymentInvoice>> {
        self.storage.get_invoices_by_customer(customer_id).await
    }

    pub async fn get_invoices_by_status(&self, status: PaymentStatus) -> Result<Vec<PaymentInvoice>> {
        self.storage.get_invoices_by_status(status).await
    }

    pub async fn get_subscription(&self, subscription_id: &str) -> Result<Option<Subscription>> {
        self.storage.get_subscription(subscription_id).await
    }

    pub async fn get_subscriptions_by_customer(&self, customer_id: &str) -> Result<Vec<Subscription>> {
        self.storage.get_subscriptions_by_customer(customer_id).await
    }

    pub async fn cancel_subscription(&self, subscription_id: &str) -> Result<Subscription> {
        let mut subscription = self.storage.get_subscription(subscription_id).await?
            .ok_or_else(|| PaymentError::SubscriptionNotFound(subscription_id.to_string()))?;

        subscription.status = SubscriptionStatus::Cancelled;
        subscription.cancelled_at = Some(chrono::Utc::now());

        self.storage.save_subscription(&subscription).await?;

        // Send notification
        self.notification_service
            .send_subscription_cancelled_notification(&subscription)
            .await?;

        Ok(subscription)
    }

    pub async fn get_stats(&self) -> PaymentStats {
        let invoices = self.storage.get_all_invoices().await.unwrap_or_default();
        let subscriptions = self.storage.get_all_subscriptions().await.unwrap_or_default();

        let total_amount: f64 = invoices.iter()
            .filter_map(|i| i.amount).sum();

        let paid_amount: f64 = invoices.iter()
            .filter(|i| i.status == PaymentStatus::Paid)
            .filter_map(|i| i.amount)
            .sum();

        let cancelled_amount: f64 = invoices.iter()
            .filter(|i| i.status == PaymentStatus::Cancelled)
            .filter_map(|i| i.amount)
            .sum();

        PaymentStats {
            total_invoices: invoices.len(),
            active_invoices: invoices.iter().filter(|i| i.status == PaymentStatus::Paid).count(),
            total_subscriptions: subscriptions.len(),
            active_subscriptions: subscriptions.iter().filter(|s| s.status == SubscriptionStatus::Active).count(),
            total_amount,
            paid_amount,
            cancelled_amount,
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PaymentStats {
    pub total_invoices: usize,
    pub active_invoices: usize,
    pub total_subscriptions: usize,
    pub active_subscriptions: usize,
    pub total_amount: f64,
    pub paid_amount: f64,
    pub cancelled_amount: f64,
}

// Webhook response
#[derive(Debug, Clone, Serialize)]
pub struct WebhookResponse {
    pub success: bool,
    pub message: String,
}