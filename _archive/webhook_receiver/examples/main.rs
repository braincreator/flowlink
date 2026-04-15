use anyhow::Result;
use clap::Parser;
use flowlink_webhook_receiver::{WebhookReceiver, WebhookReceiverConfig};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Configuration file path
    #[arg(short, long, default_value = "config.yaml")]
    config: String,

    /// Run in background mode
    #[arg(short, long)]
    background: bool,

    /// Port to bind to
    #[arg(short, long, default_value = "3002")]
    port: u16,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();

    // Setup logging
    let log_level = if args.debug {
        "debug"
    } else {
        "info"
    };

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level))
        .init();

    log::info!("Starting FlowLink Webhook Receiver");

    // Load configuration
    let config = load_config(&args.config).await?;

    // Create webhook receiver
    let receiver = WebhookReceiver::new(config).await?;

    // Start server
    if args.background {
        // Run in background
        let receiver_clone = Arc::new(receiver);
        tokio::spawn(async move {
            if let Err(e) = receiver_clone.start().await {
                log::error!("Webhook receiver failed: {}", e);
            }
        });

        log::info!("Webhook receiver started in background (PID: {})", std::process::id());
    } else {
        // Run in foreground
        if let Err(e) = receiver.start().await {
            log::error!("Webhook receiver error: {}", e);
            return Err(e);
        }
    }

    // Wait for shutdown signal
    let mut shutdown_signal = tokio::signal::ctrl_c();
    tokio::select! {
        _ = shutdown_signal => {
            log::info!("Received shutdown signal");
        }
        _ = tokio::time::sleep(Duration::from_secs(u64::MAX)) => {
            log::warn!("Webhook receiver running indefinitely");
        }
    }

    // Stop server
    if let Err(e) = receiver.stop().await {
        log::error!("Error stopping webhook receiver: {}", e);
    }

    log::info!("Webhook receiver stopped gracefully");
    Ok(())
}

async fn load_config(config_path: &str) -> Result<WebhookReceiverConfig> {
    // TODO: Implement actual config loading
    // For now, return default config

    Ok(WebhookReceiverConfig {
        port: 3002,
        public_url: "https://webhook.flow-masters.ru".to_string(),
        max_webhook_size: 10 * 1024 * 1024, // 10MB
        retention_days: 7,
        allowed_origins: vec![
            "https://flowlink.flow-masters.ru".to_string(),
            "https://webhook.flow-masters.ru".to_string(),
        ],
        hmac_secrets: vec![],
        enable_metrics: true,
        enable_storage: true,
        routing_rules: vec![],
        database: Default::default(),
        redis: Default::default(),
    })
}

// Configuration loader helpers
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_args_parsing() {
        let args = Args::try_parse_from(["webhook-receiver", "--port", "8080", "--debug"]);
        assert!(args.is_ok());
    }
}