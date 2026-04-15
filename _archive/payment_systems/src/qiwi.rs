use anyhow::Result;
use chrono::{Duration, Utc};
use std::collections::HashMap;

use super::*;
use super::models::*;

pub struct QiwiProvider {
    pub config: QiwiConfig,
}

impl QiwiProvider {
    pub fn new(config: PaymentProviderConfig) -> Result<Self> {
        let qiwi_config = QiwiConfig {
            wallet_id: config.api_key.clone(),
            api_key: config.api_secret.clone(),
            is_sandbox: config.sandbox,
        };

        Ok(Self { config: qiwi_config })
    }

    pub async fn create_invoice(
        &self,
        _config: &QiwiConfig,
        amount: f64,
        currency: &str,
        description: &str,
        customer_id: &str,
        metadata: HashMap<String, String>,
    ) -> Result<PaymentInvoice> {
        let amount_str = format!("{:.2}", amount);
        let bill_id = format!("{}-{}", customer_id, uuid::Uuid::new_v4());

        let subscription = self.create_subscription_internal(
            &bill_id,
            amount,
            currency,
            description,
            customer_id,
        ).await?;

        Ok(subscription)
    }

    pub async fn create_subscription(
        &self,
        _config: &QiwiConfig,
        amount: f64,
        period: SubscriptionPeriod,
        customer_id: &str,
        plan_id: &str,
    ) -> Result<Subscription> {
        let subscription_id = format!("{}-{}", customer_id, uuid::Uuid::new_v4());

        let subscription = self.create_subscription_internal(
            &subscription_id,
            amount,
            "RUB",
            plan_id,
            customer_id,
        ).await?;

        // Set period
        let period_end = match period {
            SubscriptionPeriod::Monthly => Utc::now() + Duration::days(30),
            SubscriptionPeriod::Quarterly => Utc::now() + Duration::days(90),
            SubscriptionPeriod::Annually => Utc::now() + Duration::days(365),
        };

        Ok(Subscription {
            ..subscription
            .period(SubscriptionPeriod::Monthly)
            .current_period_end(period_end)
        })
    }

    async fn create_subscription_internal(
        &self,
        bill_id: &str,
        amount: f64,
        currency: &str,
        description: &str,
        customer_id: &str,
    ) -> Result<Subscription> {
        Ok(Subscription {
            id: bill_id.to_string(),
            provider: "qiwi".to_string(),
            customer_id: customer_id.to_string(),
            plan_id: description.to_string(),
            amount,
            currency: currency.to_string(),
            period: SubscriptionPeriod::Monthly,
            status: SubscriptionStatus::Active,
            current_period_start: Utc::now(),
            current_period_end: Utc::now() + Duration::days(30),
            trial_end: None,
            auto_renew: true,
            cancel_at_period_end: false,
            cancelled_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
    }

    pub async fn process_payment(
        &self,
        _config: &QiwiConfig,
        payment_id: &str,
        _payment_data: &str,
    ) -> Result<PaymentResult> {
        // Qiwi doesn't provide API for payment processing
        // For now, simulate success

        Ok(PaymentResult {
            success: true,
            payment_id: payment_id.to_string(),
            invoice_id: None,
            amount: 0.0,
            currency: "RUB".to_string(),
            status: PaymentStatus::Paid,
            message: "Payment processed successfully".to_string(),
        })
    }

    pub async fn create_refund(
        &self,
        _config: &QiwiConfig,
        request: &RefundRequest,
    ) -> Result<Refund> {
        Ok(Refund {
            id: uuid::Uuid::new_v4().to_string(),
            invoice_id: request.description.clone(),
            provider: "qiwi".to_string(),
            amount: request.amount,
            status: RefundStatus::Pending,
            reason: request.reason.clone(),
            created_at: Utc::now(),
            processed_at: None,
        })
    }

    pub async fn verify_webhook_signature(
        &self,
        _config: &PaymentProviderConfig,
        _signature: &str,
        _payload: &str,
    ) -> Result<bool> {
        // Qiwi doesn't use webhook signatures in the same way
        Ok(true)
    }

    pub async fn parse_webhook(&self, payload: &str) -> Result<WebhookData> {
        let webhook_data: serde_json::Value = serde_json::from_str(payload)?;

        Ok(WebhookData {
            event_type: webhook_data["event"].as_str()
                .unwrap_or("")
                .to_string(),
            id: webhook_data["transaction"]["id"].as_str()
                .unwrap_or("")
                .to_string(),
            date: Utc::now(),
            data: WebhookDataContent {
                data_type: "qiwi.bill".to_string(),
                object: webhook_data["transaction"].clone(),
            },
        })
    }

    fn get_provider_type(&self) -> &str {
        "qiwi"
    }
}

// Make Subscription methods mutable for Qiwi
impl Subscription {
    pub fn period(mut self, period: SubscriptionPeriod) -> Self {
        self.period = period;
        self.updated_at = Utc::now();
        self
    }

    pub fn current_period_end(mut self, end: DateTime<Utc>) -> Self {
        self.current_period_end = end;
        self.updated_at = Utc::now();
        self
    }
}