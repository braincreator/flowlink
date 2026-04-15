use anyhow::Result;
use chrono::{Duration, Utc};
use std::collections::HashMap;

use super::*;
use super::models::*;

pub struct YooKassaProvider {
    pub config: YooKassaConfig,
}

impl YooKassaProvider {
    pub fn new(config: PaymentProviderConfig) -> Result<Self> {
        let yookassa_config = YooKassaConfig {
            shop_id: config.api_key.clone(),
            secret_key: config.api_secret.clone(),
            is_sandbox: config.sandbox,
        };

        Ok(Self { config: yookassa_config })
    }

    pub async fn create_invoice(
        &self,
        _config: &YooKassaConfig,
        amount: f64,
        currency: &str,
        description: &str,
        customer_id: &str,
        metadata: HashMap<String, String>,
    ) -> Result<PaymentInvoice> {
        // Build API URL based on environment
        let base_url = if self.config.is_sandbox {
            "https://yookassa.ru/api/v3/test"
        } else {
            "https://yookassa.ru/api/v3"
        };

        // Construct amount string
        let amount_str = format!("{:.2}", amount);
        let currency_str = currency.to_uppercase();

        // Create invoice request
        let request = serde_json::json!({
            "amount": {
                "value": amount_str,
                "currency": currency_str
            },
            "capture": true,
            "confirmation": {
                "type": "redirect",
                "return_url": format!("{}/payment/success", metadata.get("return_url").unwrap_or(&"".to_string()))
            },
            "description": description,
            "metadata": metadata,
            "customer": {
                "id": customer_id
            }
        });

        // Call YooKassa API
        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/payments", base_url))
            .header("Idempotence-Key", uuid::Uuid::new_v4().to_string())
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.config.secret_key))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("YooKassa API error: {}", error_text));
        }

        let payment_response: serde_json::Value = response.json().await?;

        let payment_id = payment_response["id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Payment ID not found"))?
            .to_string();

        let amount = payment_response["amount"]["value"]
            .as_str()
            .and_then(|s| s.parse::<f64>().ok());

        let status = match payment_response["status"].as_str() {
            Some("pending") => PaymentStatus::Pending,
            Some("succeeded") => PaymentStatus::Paid,
            Some("canceled") => PaymentStatus::Cancelled,
            Some("waiting_for_capture") => PaymentStatus::Processing,
            _ => PaymentStatus::Pending,
        };

        let pay_url = payment_response["confirmation"]["confirmation_url"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(PaymentInvoice {
            id: uuid::Uuid::new_v4().to_string(),
            provider: "yookassa".to_string(),
            customer_id: customer_id.to_string(),
            amount,
            currency: currency_str,
            description: description.to_string(),
            status,
            payment_id: Some(payment_id.clone()),
            payment_url: Some(pay_url),
            metadata,
            created_at: Utc::now(),
            paid_at: if status == PaymentStatus::Paid {
                Some(Utc::now())
            } else {
                None
            },
            failed_at: None,
        })
    }

    pub async fn create_subscription(
        &self,
        _config: &YooKassaConfig,
        amount: f64,
        period: SubscriptionPeriod,
        customer_id: &str,
        plan_id: &str,
    ) -> Result<Subscription> {
        // YooKassa doesn't have built-in subscription support
        // We'll create a recurring payment plan manually

        let subscription = match period {
            SubscriptionPeriod::Monthly => {
                Subscription::new(
                    uuid::Uuid::new_v4().to_string(),
                    "yookassa".to_string(),
                    customer_id.to_string(),
                    plan_id.to_string(),
                    amount,
                    "RUB".to_string(),
                    SubscriptionPeriod::Monthly,
                    SubscriptionStatus::Active,
                    Utc::now(),
                    Utc::now() + Duration::days(30),
                    None,
                    true,
                    false,
                    None,
                    Utc::now(),
                    Utc::now(),
                )
            }
            SubscriptionPeriod::Quarterly => {
                Subscription::new(
                    uuid::Uuid::new_v4().to_string(),
                    "yookassa".to_string(),
                    customer_id.to_string(),
                    plan_id.to_string(),
                    amount,
                    "RUB".to_string(),
                    SubscriptionPeriod::Quarterly,
                    SubscriptionStatus::Active,
                    Utc::now(),
                    Utc::now() + Duration::days(90),
                    None,
                    true,
                    false,
                    None,
                    Utc::now(),
                    Utc::now(),
                )
            }
            SubscriptionPeriod::Annually => {
                Subscription::new(
                    uuid::Uuid::new_v4().to_string(),
                    "yookassa".to_string(),
                    customer_id.to_string(),
                    plan_id.to_string(),
                    amount,
                    "RUB".to_string(),
                    SubscriptionPeriod::Annually,
                    SubscriptionStatus::Active,
                    Utc::now(),
                    Utc::now() + Duration::days(365),
                    None,
                    true,
                    false,
                    None,
                    Utc::now(),
                    Utc::now(),
                )
            }
        };

        Ok(subscription)
    }

    pub async fn process_payment(
        &self,
        _config: &YooKassaConfig,
        payment_id: &str,
        payment_data: &str,
    ) -> Result<PaymentResult> {
        let base_url = if self.config.is_sandbox {
            "https://yookassa.ru/api/v3/test"
        } else {
            "https://yookassa.ru/api/v3"
        };

        // Call YooKassa API to get payment details
        let client = reqwest::Client::new();
        let response = client
            .get(format!("{}/payments/{}", base_url, payment_id))
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.config.secret_key))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("YooKassa API error: {}", error_text));
        }

        let payment_response: serde_json::Value = response.json().await?;

        let status = match payment_response["status"].as_str() {
            Some("succeeded") => PaymentStatus::Paid,
            Some("canceled") => PaymentStatus::Cancelled,
            Some("failed") => PaymentStatus::Failed,
            _ => PaymentStatus::Pending,
        };

        Ok(PaymentResult {
            success: status == PaymentStatus::Paid,
            payment_id: payment_id.to_string(),
            invoice_id: None,
            amount: payment_response["amount"]["value"]
                .as_str()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0),
            currency: payment_response["amount"]["currency"]
                .as_str()
                .unwrap_or("RUB")
                .to_string(),
            status,
            message: status.to_string(),
        })
    }

    pub async fn create_refund(
        &self,
        _config: &YooKassaConfig,
        request: &RefundRequest,
    ) -> Result<Refund> {
        // YooKassa doesn't have built-in refund API
        // For now, we'll simulate a refund

        let refund_id = uuid::Uuid::new_v4().to_string();

        Ok(Refund {
            id: refund_id,
            invoice_id: request.description.clone(),
            provider: "yookassa".to_string(),
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
        signature: &str,
        payload: &str,
    ) -> Result<bool> {
        // YooKassa uses HMAC-SHA256 for webhook signature verification
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        type HmacSha256 = Hmac<Sha256>;

        let secret_key = self.config.secret_key.as_bytes();
        let mut mac = HmacSha256::new_from_slice(secret_key)
            .expect("Failed to create HMAC key");

        mac.update(payload.as_bytes());

        let expected_signature = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));

        // YooKassa sends signature in header "YooKassa-Notification-Signature"
        Ok(expected_signature == signature)
    }

    pub async fn parse_webhook(&self, payload: &str) -> Result<WebhookData> {
        let webhook_data: serde_json::Value = serde_json::from_str(payload)?;

        Ok(WebhookData {
            event_type: webhook_data["event"].as_str()
                .unwrap_or("")
                .to_string(),
            id: webhook_data["object"]["id"].as_str()
                .unwrap_or("")
                .to_string(),
            date: Utc::now(), // YooKassa doesn't provide event date in webhook
            data: WebhookDataContent {
                data_type: webhook_data["object"]["type"].as_str()
                    .unwrap_or("")
                    .to_string(),
                object: webhook_data["object"]["payment"]
                    .clone()
                    .unwrap_or_else(|| webhook_data["object"]["invoice"].clone()),
            },
        })
    }

    fn get_provider_type(&self) -> &str {
        "yookassa"
    }
}

impl Subscription {
    fn new(
        id: String,
        provider: String,
        customer_id: String,
        plan_id: String,
        amount: f64,
        currency: String,
        period: SubscriptionPeriod,
        status: SubscriptionStatus,
        current_period_start: DateTime<Utc>,
        current_period_end: DateTime<Utc>,
        trial_end: Option<DateTime<Utc>>,
        auto_renew: bool,
        cancel_at_period_end: bool,
        cancelled_at: Option<DateTime<Utc>>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            provider,
            customer_id,
            plan_id,
            amount,
            currency,
            period,
            status,
            current_period_start,
            current_period_end,
            trial_end,
            auto_renew,
            cancel_at_period_end,
            cancelled_at,
            created_at,
            updated_at,
        }
    }
}