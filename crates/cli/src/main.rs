// FlowLink CLI — unified command for agent and relay

use clap::{Parser, Subcommand};
use log::info;

#[derive(Parser)]
#[command(name = "flowlink", version, about = "FlowLink — secure remote agent management")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the agent (connects to relay)
    Agent {
        /// Config file path
        #[arg(short, long, default_value = "flowlink.json")]
        config: String,
    },
    /// Start the relay server
    Relay {
        /// Config file path
        #[arg(short, long, default_value = "relay.json")]
        config: String,
        /// Bind address override
        #[arg(long)]
        addr: Option<String>,
    },
    /// Generate a new keypair
    Keygen {
        /// Output file (prints to stdout if omitted)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Encrypt a message for a peer
    Encrypt {
        /// Recipient's public key (base64)
        #[arg(long)]
        peer_key: String,
        /// Message file (stdin if omitted)
        #[arg(short, long)]
        input: Option<String>,
    },
    /// Version info
    Version,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.verbose {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    } else {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    }

    match cli.command {
        Commands::Agent { config } => {
            let cfg = flowlink_core::config::AgentConfig::load(&config)?;
            let agent = flowlink_agent::Agent::new(cfg);
            agent.run().await
        }
        Commands::Relay { config, addr } => {
            let mut cfg = flowlink_core::config::RelayConfig::load(&config)?;
            if let Some(addr) = addr {
                cfg.http_addr = addr.parse()?;
            }
            let relay = flowlink_relay::Relay::new(cfg);
            relay.run().await
        }
        Commands::Keygen { output } => {
            let keypair = flowlink_crypto::KeyPair::generate();
            let json = serde_json::to_string_pretty(&keypair)?;
            match output {
                Some(path) => std::fs::write(path, &json)?,
                None => println!("{json}"),
            }
            Ok(())
        }
        Commands::Encrypt { peer_key: _, input: _ } => {
            // TODO: read input, encrypt with peer key
            info!("Encrypt not yet implemented");
            Ok(())
        }
        Commands::Version => {
            println!("flowlink {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}
