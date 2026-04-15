use anyhow::Result;
use sqlx::postgres::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::*;
use super::models::*;

pub struct PaymentStorage {
    pub pool: Arc<PgPool>,
}

impl PaymentStorage {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    pub async fn create_tables(&self) -> Result<()> {
        let create_tables_sql = r#"
            CREATE TABLE IF NOT EXISTS payment_provider_configs (
                provider TEXT PRIMARY KEY,
                api_key TEXT NOT NULL,
                api_secret TEXT NOT NULL,
                sandbox BOOLEAN NOT NULL DEFAULT TRUE,
                created_at TIMESTAMP NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMP NOT NULL DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS payment_notifications (
                id SERIAL PRIMARY KEY,
                invoice_id TEXT,
                subscription_id TEXT,
                notification_type TEXT NOT NULL,
                status TEXT NOT NULL,
                error_message TEXT,
                created_at TIMESTAMP NOT NULL DEFAULT NOW()
            );
        "#;

        sqlx::query(create_tables_sql)
            .execute(self.pool.clone())
            .await?;

        log::info!("Payment storage tables created successfully");
        Ok(())
    }

    pub async fn save_provider_config(&self, config: &PaymentProviderConfig) -> Result<()> {
        let insert_sql = r#"
            INSERT INTO payment_provider_configs (
                provider, api_key, api_secret, sandbox
            )
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (provider) DO UPDATE SET
                api_key = EXCLUDED.api_key,
                api_secret = EXCLUDED.api_secret,
                sandbox = EXCLUDED.sandbox,
                updated_at = NOW()
        "#;

        sqlx::query(insert_sql)
            .bind(&config.provider_type)
            .bind(&config.api_key)
            .bind(&config.api_secret)
            .bind(config.sandbox)
            .execute(self.pool.clone())
            .await?;

        Ok(())
    }

    pub async fn get_provider_config(&self, provider_type: &str) -> Result<PaymentProviderConfig> {
        let query = r#"
            SELECT provider, api_key, api_secret, sandbox
            FROM payment_provider_configs
            WHERE provider = $1
        "#;

        let row = sqlx::query(query)
            .bind(provider_type)
            .fetch_optional(self.pool.clone())
            .await?;

        match row {
            Some(row) => {
                Ok(PaymentProviderConfig {
                    provider_type: row.try_get("provider")?,
                    api_key: row.try_get("api_key")?,
                    api_secret: row.try_get("api_secret")?,
                    sandbox: row.try_get("sandbox")?,
                })
            }
            None => Err(anyhow::anyhow!("Provider config not found: {}", provider_type)),
        }
    }

    pub async fn get_all_provider_configs(&self) -> Result<Vec<PaymentProviderConfig>> {
        let query = r#"
            SELECT provider, api_key, api_secret, sandbox
            FROM payment_provider_configs
        "#;

        let rows = sqlx::query(query).fetch_all(self.pool.clone()).await?;

        let mut configs = Vec::new();

        for row in rows {
            configs.push(PaymentProviderConfig {
                provider_type: row.try_get("provider")?,
                api_key: row.try_get("api_key")?,
                api_secret: row.try_get("api_secret")?,
                sandbox: row.try_get("sandbox")?,
            });
        }

        Ok(configs)
    }

    pub async fn save_invoice(&self, invoice: &PaymentInvoice) -> Result<()> {
        use super::invoice::InvoiceStorage;
        let invoice_storage = InvoiceStorage::new(self.pool.clone());

        invoice_storage.save_invoice(invoice).await
    }

    pub async fn get_invoice(&self, invoice_id: &str) -> Result<Option<PaymentInvoice>> {
        use super::invoice::InvoiceStorage;
        let invoice_storage = InvoiceStorage::new(self.pool.clone());

        invoice_storage.get_invoice(invoice_id).await
    }

    pub async fn get_invoices_by_customer(&self, customer_id: &str) -> Result<Vec<PaymentInvoice>> {
        use super::invoice::InvoiceStorage;
        let invoice_storage = InvoiceStorage::new(self.pool.clone());

        invoice_storage.get_invoices_by_customer(customer_id).await
    }

    pub async fn get_invoices_by_status(&self, status: PaymentStatus) -> Result<Vec<PaymentInvoice>> {
        use super::invoice::InvoiceStorage;
        let invoice_storage = InvoiceStorage::new(self.pool.clone());

        invoice_storage.get_invoices_by_status(status).await
    }

    pub async fn get_all_invoices(&self) -> Result<Vec<PaymentInvoice>> {
        use super::invoice::InvoiceStorage;
        let invoice_storage = InvoiceStorage::new(self.pool.clone());

        invoice_storage.get_all_invoices().await
    }

    pub async fn update_invoice_status(&self, invoice_id: &str, status: PaymentStatus) -> Result<()> {
        use super::invoice::InvoiceStorage;
        let invoice_storage = InvoiceStorage::new(self.pool.clone());

        invoice_storage.update_invoice_status(invoice_id, status).await
    }

    pub async fn save_subscription(&self, subscription: &Subscription) -> Result<()> {
        use super::subscription::SubscriptionStorage;
        let subscription_storage = SubscriptionStorage::new(self.pool.clone());

        subscription_storage.save_subscription(subscription).await
    }

    pub async fn get_subscription(&self, subscription_id: &str) -> Result<Option<Subscription>> {
        use super::subscription::SubscriptionStorage;
        let subscription_storage = SubscriptionStorage::new(self.pool.clone());

        subscription_storage.get_subscription(subscription_id).await
    }

    pub async fn get_subscriptions_by_customer(&self, customer_id: &str) -> Result<Vec<Subscription>> {
        use super::subscription::SubscriptionStorage;
        let subscription_storage = SubscriptionStorage::new(self.pool.clone());

        subscription_storage.get_subscriptions_by_customer(customer_id).await
    }

    pub async fn get_all_subscriptions(&self) -> Result<Vec<Subscription>> {
        use super::subscription::SubscriptionStorage;
        let subscription_storage = SubscriptionStorage::new(self.pool.clone());

        subscription_storage.get_all_subscriptions().await
    }
}

// In-memory storage for testing
pub struct InMemoryPaymentStorage {
    pub provider_configs: Arc<RwLock<HashMap<String, PaymentProviderConfig>>>,
    pub invoices: Arc<RwLock<HashMap<String, PaymentInvoice>>>,
    pub subscriptions: Arc<RwLock<HashMap<String, Subscription>>>,
}

impl InMemoryPaymentStorage {
    pub fn new() -> Self {
        Self {
            provider_configs: Arc::new(RwLock::new(HashMap::new())),
            invoices: Arc::new(RwLock::new(HashMap::new())),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn save_provider_config(&self, config: &PaymentProviderConfig) -> Result<()> {
        self.provider_configs.write().await.insert(config.provider_type.clone(), config.clone());
        Ok(())
    }

    pub async fn get_provider_config(&self, provider_type: &str) -> Result<PaymentProviderConfig> {
        Ok(self.provider_configs.read().await.get(provider_type)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Provider config not found: {}", provider_type))?)
    }

    pub async fn save_invoice(&self, invoice: &PaymentInvoice) -> Result<()> {
        self.invoices.write().await.insert(invoice.id.clone(), invoice.clone());
        Ok(())
    }

    pub async fn get_invoice(&self, invoice_id: &str) -> Result<Option<PaymentInvoice>> {
        Ok(self.invoices.read().await.get(invoice_id).cloned())
    }

    pub async fn get_invoices_by_customer(&self, customer_id: &str) -> Result<Vec<PaymentInvoice>> {
        Ok(self.invoices.read().await.values()
            .filter(|i| i.customer_id == customer_id)
            .cloned()
            .collect())
    }

    pub async fn get_invoices_by_status(&self, status: PaymentStatus) -> Result<Vec<PaymentInvoice>> {
        Ok(self.invoices.read().await.values()
            .filter(|i| i.status == status)
            .cloned()
            .collect())
    }

    pub async fn get_all_invoices(&self) -> Result<Vec<PaymentInvoice>> {
        Ok(self.invoices.read().await.values().cloned().collect())
    }

    pub async fn save_subscription(&self, subscription: &Subscription) -> Result<()> {
        self.subscriptions.write().await.insert(subscription.id.clone(), subscription.clone());
        Ok(())
    }

    pub async fn get_subscription(&self, subscription_id: &str) -> Result<Option<Subscription>> {
        Ok(self.subscriptions.read().await.get(subscription_id).cloned())
    }

    pub async fn get_subscriptions_by_customer(&self, customer_id: &str) -> Result<Vec<Subscription>> {
        Ok(self.subscriptions.read().await.values()
            .filter(|s| s.customer_id == customer_id)
            .cloned()
            .collect())
    }

    pub async fn get_all_subscriptions(&self) -> Result<Vec<Subscription>> {
        Ok(self.subscriptions.read().await.values().cloned().collect())
    }
}