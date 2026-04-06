// FlowLink CLI — unified command for agent and relay

use clap::{Parser, Subcommand};
use log::info;
use std::io::{self, Read};

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
        #[arg(short, long, default_value = "flowlink.json")]
        config: String,
    },
    /// Start the relay server
    Relay {
        #[arg(short, long, default_value = "relay.json")]
        config: String,
        #[arg(long)]
        addr: Option<String>,
    },
    /// Generate a new X25519 keypair
    Keygen {
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Encrypt a message for a peer's public key
    Encrypt {
        /// Recipient's public key (base64)
        #[arg(long)]
        peer_key: String,
        /// Input file (stdin if omitted)
        #[arg(short, long)]
        input: Option<String>,
    },
    /// Decrypt a message using your keypair
    Decrypt {
        /// Your keypair JSON file
        #[arg(long)]
        keypair: String,
        /// Encrypted envelope file (stdin if omitted)
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
        Commands::Encrypt { peer_key, input } => {
            let plaintext = read_input(&input)?;
            let my_keypair = flowlink_crypto::KeyPair::generate();
            let envelope = flowlink_crypto::encrypt(&my_keypair, &peer_key, &plaintext)?;
            let json = serde_json::to_string_pretty(&envelope)?;
            println!("{json}");
            Ok(())
        }
        Commands::Decrypt { keypair, input } => {
            let kp_json = std::fs::read_to_string(&keypair)?;
            let my_keypair: flowlink_crypto::KeyPair = serde_json::from_str(&kp_json)?;
            let envelope_bytes = read_input(&input)?;
            let envelope_json = String::from_utf8(envelope_bytes)?;
            let envelope: flowlink_crypto::EncryptedEnvelope = serde_json::from_str(&envelope_json)?;
            let plaintext = flowlink_crypto::decrypt(&my_keypair, &envelope)?;
            print!("{}", String::from_utf8_lossy(&plaintext));
            Ok(())
        }
        Commands::Version => {
            println!("flowlink {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}

fn read_input(path: &Option<String>) -> anyhow::Result<Vec<u8>> {
    match path {
        Some(p) => Ok(std::fs::read(p)?),
        None => {
            let mut buf = Vec::new();
            io::stdin().read_to_end(&mut buf)?;
            Ok(buf)
        }
    }
}
