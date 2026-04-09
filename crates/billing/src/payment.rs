//! Payment methods and configuration
//!
//! Supported Russian payment methods:
//! - SBP (Система Быстрых Платежей)
//! - Bank cards (via Точка acquiring)
//! - Invoice (счет на оплату для юрлиц)

use serde::{Deserialize, Serialize};

/// Payment method types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaymentMethod {
    /// Система Быстрых Платежей
    Sbp,
    /// Bank card (Visa/Mir/Mastercard via acquiring)
    Card,
    /// Bank transfer invoice (для юрлиц/ИП)
    BankTransfer,
    /// Manual/admin adjustment
    Admin,
}

impl PaymentMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            PaymentMethod::Sbp => "sbp",
            PaymentMethod::Card => "card",
            PaymentMethod::BankTransfer => "bank_transfer",
            PaymentMethod::Admin => "admin",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            PaymentMethod::Sbp => "СБП",
            PaymentMethod::Card => "Банковская карта",
            PaymentMethod::BankTransfer => "Банковский перевод",
            PaymentMethod::Admin => "Административный",
        }
    }
}

impl std::fmt::Display for PaymentMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Payment status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaymentStatus {
    /// Payment initiated
    Pending,
    /// Payment completed successfully
    Completed,
    /// Payment failed
    Failed,
    /// Payment refunded
    Refunded,
    /// Payment expired
    Expired,
}

/// SBP payment provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SbpConfig {
    /// Точка Банк terminal ID
    pub terminal_key: String,
    /// Точка Банк secret key
    pub secret_key: String,
    /// SBP payment type ID from Точка
    pub payment_type_id: String,
    /// Callback URL for payment notifications
    pub callback_url: String,
    /// Success redirect URL
    pub success_url: String,
    /// Fail redirect URL
    pub fail_url: String,
}

/// Bank details for BankTransfer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankDetails {
    /// Company name
    pub company_name: String,
    /// INN
    pub inn: String,
    /// KPP
    pub kpp: String,
    /// OGRN
    pub ogrn: String,
    /// Bank account (расчётный счёт)
    pub account: String,
    /// Bank name
    pub bank_name: String,
    /// BIK
    pub bik: String,
    /// Correspondent account (корр. счёт)
    pub kor_account: String,
}

/// Payment configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentConfig {
    /// Whether payments are enabled
    pub enabled: bool,
    /// SBP configuration
    pub sbp: Option<SbpConfig>,
    /// Bank details for invoices
    pub bank_details: Option<BankDetails>,
    /// Payment timeout in seconds (default: 24h)
    pub payment_timeout_secs: u64,
    /// Overage pricing: extra API request in kopecks
    pub overage_request_price_kopecks: u64,
    /// Overage pricing: extra 1K tokens in kopecks
    pub overage_token_price_kopecks: u64,
    /// Minimum top-up amount in kopecks
    pub min_topup_kopecks: u64,
    /// Maximum top-up amount in kopecks
    pub max_topup_kopecks: u64,
}

impl Default for PaymentConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            sbp: None,
            bank_details: None,
            payment_timeout_secs: 86400, // 24 hours
            overage_request_price_kopecks: 10, // 0.10 RUB per extra request
            overage_token_price_kopecks: 50, // 0.50 RUB per 1K extra tokens
            min_topup_kopecks: 10_000, // 100 RUB
            max_topup_kopecks: 1_000_000, // 10,000 RUB
        }
    }
}

/// Available payment methods based on config
impl PaymentConfig {
    /// Get list of available payment methods
    pub fn available_methods(&self) -> Vec<PaymentMethod> {
        let mut methods = vec![];
        if self.enabled {
            methods.push(PaymentMethod::Card);
            if self.sbp.is_some() {
                methods.push(PaymentMethod::Sbp);
            }
            if self.bank_details.is_some() {
                methods.push(PaymentMethod::BankTransfer);
            }
        }
        methods
    }

    /// Check if a payment method is available
    pub fn is_available(&self, method: PaymentMethod) -> bool {
        match method {
            PaymentMethod::Sbp => self.enabled && self.sbp.is_some(),
            PaymentMethod::Card => self.enabled,
            PaymentMethod::BankTransfer => self.enabled && self.bank_details.is_some(),
            PaymentMethod::Admin => true,
        }
    }

    /// Format kopecks to RUB string
    pub fn format_rub(kopecks: i64) -> String {
        let rubles = kopecks as f64 / 100.0;
        format!("{:.2} ₽", rubles)
    }

    /// Получить конфиг SBP (Точка Банк)
    pub fn sbp_config(&self) -> Option<&SbpConfig> {
        self.sbp.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payment_method_display() {
        assert_eq!(PaymentMethod::Sbp.as_str(), "sbp");
        assert_eq!(PaymentMethod::Sbp.display_name(), "СБП");
        assert_eq!(PaymentMethod::Card.display_name(), "Банковская карта");
        assert_eq!(PaymentMethod::BankTransfer.display_name(), "Банковский перевод");
    }

    #[test]
    fn test_default_config() {
        let config = PaymentConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.available_methods().len(), 0); // no methods when disabled
    }

    #[test]
    fn test_enabled_config() {
        let config = PaymentConfig {
            enabled: true,
            sbp: Some(SbpConfig {
                terminal_key: "test".to_string(),
                secret_key: "test".to_string(),
                payment_type_id: "test".to_string(),
                callback_url: "https://test.com/callback".to_string(),
                success_url: "https://test.com/success".to_string(),
                fail_url: "https://test.com/fail".to_string(),
            }),
            bank_details: Some(BankDetails {
                company_name: "ООО Тест".to_string(),
                inn: "1234567890".to_string(),
                kpp: "123456789".to_string(),
                ogrn: "1234567890123".to_string(),
                account: "40702810000000000001".to_string(),
                bank_name: "Точка Банк".to_string(),
                bik: "044525999".to_string(),
                kor_account: "30101810000000000001".to_string(),
            }),
            ..PaymentConfig::default()
        };

        let methods = config.available_methods();
        assert_eq!(methods.len(), 3); // Card + SBP + BankTransfer
        assert!(config.is_available(PaymentMethod::Sbp));
        assert!(config.is_available(PaymentMethod::BankTransfer));
        assert!(config.is_available(PaymentMethod::Card));
    }

    #[test]
    fn test_format_rub() {
        assert_eq!(PaymentConfig::format_rub(29990), "299.90 ₽");
        assert_eq!(PaymentConfig::format_rub(0), "0.00 ₽");
        assert_eq!(PaymentConfig::format_rub(-100), "-1.00 ₽");
    }
}
