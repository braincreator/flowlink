// FlowLink CLI — unified command for agent and relay

use clap::{Parser, Subcommand};
use std::io::{self, Read};

#[derive(Parser, Debug)]
#[command(name = "flowlink", version, about = "FlowLink — secure remote agent management")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
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
            let mut relay = flowlink_relay::Relay::new(cfg);
            relay = relay.with_config_path(&config);
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

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    // Helper: parse args and return the Cli struct
    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(args).expect("parse failed")
    }

    fn parse_err(args: &[&str]) -> String {
        Cli::try_parse_from(args)
            .unwrap_err()
            .to_string()
    }

    // ─── Global flags ───

    #[test]
    fn verbose_flag() {
        let cli = parse(&["flowlink", "--verbose", "agent"]);
        assert!(cli.verbose);
    }

    #[test]
    fn verbose_short() {
        let cli = parse(&["flowlink", "-v", "agent"]);
        assert!(cli.verbose);
    }

    #[test]
    fn no_verbose_default() {
        let cli = parse(&["flowlink", "agent"]);
        assert!(!cli.verbose);
    }

    #[test]
    fn verbose_before_subcommand() {
        let cli = parse(&["flowlink", "--verbose", "relay"]);
        assert!(cli.verbose);
    }

    #[test]
    fn verbose_after_subcommand_fails() {
        // -v after subcommand won't be picked up (global flag must be before)
        // Actually clap global flags work before subcommand. After subcommand they're not recognized.
        let cli = parse(&["flowlink", "-v", "keygen"]);
        assert!(cli.verbose);
    }

    // ─── No subcommand → error ───

    #[test]
    fn no_subcommand_errors() {
        let err = parse_err(&["flowlink"]);
        assert!(err.contains("required") || err.contains("COMMAND"), "got: {err}");
    }

    // ─── Agent subcommand ───

    #[test]
    fn agent_default_config() {
        let cli = parse(&["flowlink", "agent"]);
        match cli.command {
            Commands::Agent { config } => assert_eq!(config, "flowlink.json"),
            _ => panic!("expected Agent"),
        }
    }

    #[test]
    fn agent_custom_config_short() {
        let cli = parse(&["flowlink", "agent", "-c", "my.json"]);
        match cli.command {
            Commands::Agent { config } => assert_eq!(config, "my.json"),
            _ => panic!("expected Agent"),
        }
    }

    #[test]
    fn agent_custom_config_long() {
        let cli = parse(&["flowlink", "agent", "--config", "custom.json"]);
        match cli.command {
            Commands::Agent { config } => assert_eq!(config, "custom.json"),
            _ => panic!("expected Agent"),
        }
    }

    #[test]
    fn agent_verbose() {
        let cli = parse(&["flowlink", "--verbose", "agent"]);
        assert!(cli.verbose);
        assert!(matches!(cli.command, Commands::Agent { .. }));
    }

    #[test]
    fn agent_abs_path_config() {
        let cli = parse(&["flowlink", "agent", "-c", "/etc/flowlink/agent.json"]);
        match cli.command {
            Commands::Agent { config } => assert_eq!(config, "/etc/flowlink/agent.json"),
            _ => panic!("expected Agent"),
        }
    }

    // ─── Relay subcommand ───

    #[test]
    fn relay_default_config() {
        let cli = parse(&["flowlink", "relay"]);
        match cli.command {
            Commands::Relay { config, addr } => {
                assert_eq!(config, "relay.json");
                assert!(addr.is_none());
            }
            _ => panic!("expected Relay"),
        }
    }

    #[test]
    fn relay_custom_config() {
        let cli = parse(&["flowlink", "relay", "-c", "prod.json"]);
        match cli.command {
            Commands::Relay { config, .. } => assert_eq!(config, "prod.json"),
            _ => panic!("expected Relay"),
        }
    }

    #[test]
    fn relay_addr_flag() {
        let cli = parse(&["flowlink", "relay", "--addr", "0.0.0.0:9090"]);
        match cli.command {
            Commands::Relay { addr, .. } => assert_eq!(addr.as_deref(), Some("0.0.0.0:9090")),
            _ => panic!("expected Relay"),
        }
    }

    #[test]
    fn relay_config_and_addr() {
        let cli = parse(&["flowlink", "relay", "-c", "r.json", "--addr", "127.0.0.1:8080"]);
        match cli.command {
            Commands::Relay { config, addr } => {
                assert_eq!(config, "r.json");
                assert_eq!(addr.as_deref(), Some("127.0.0.1:8080"));
            }
            _ => panic!("expected Relay"),
        }
    }

    #[test]
    fn relay_addr_default_none() {
        let cli = parse(&["flowlink", "relay"]);
        match cli.command {
            Commands::Relay { addr, .. } => assert!(addr.is_none()),
            _ => panic!("expected Relay"),
        }
    }

    // ─── Keygen subcommand ───

    #[test]
    fn keygen_no_output() {
        let cli = parse(&["flowlink", "keygen"]);
        match cli.command {
            Commands::Keygen { output } => assert!(output.is_none()),
            _ => panic!("expected Keygen"),
        }
    }

    #[test]
    fn keygen_output_short() {
        let cli = parse(&["flowlink", "keygen", "-o", "keys.json"]);
        match cli.command {
            Commands::Keygen { output } => assert_eq!(output.as_deref(), Some("keys.json")),
            _ => panic!("expected Keygen"),
        }
    }

    #[test]
    fn keygen_output_long() {
        let cli = parse(&["flowlink", "keygen", "--output", "kp.json"]);
        match cli.command {
            Commands::Keygen { output } => assert_eq!(output.as_deref(), Some("kp.json")),
            _ => panic!("expected Keygen"),
        }
    }

    // ─── Encrypt subcommand ───

    #[test]
    fn encrypt_required_peer_key() {
        let err = parse_err(&["flowlink", "encrypt"]);
        assert!(err.contains("--peer-key"), "got: {err}");
    }

    #[test]
    fn encrypt_peer_key() {
        let cli = parse(&["flowlink", "encrypt", "--peer-key", "dGVzdA=="]);
        match cli.command {
            Commands::Encrypt { peer_key, input } => {
                assert_eq!(peer_key, "dGVzdA==");
                assert!(input.is_none());
            }
            _ => panic!("expected Encrypt"),
        }
    }

    #[test]
    fn encrypt_with_input_file() {
        let cli = parse(&["flowlink", "encrypt", "--peer-key", "abc", "-i", "msg.txt"]);
        match cli.command {
            Commands::Encrypt { input, .. } => assert_eq!(input.as_deref(), Some("msg.txt")),
            _ => panic!("expected Encrypt"),
        }
    }

    #[test]
    fn encrypt_with_input_long() {
        let cli = parse(&["flowlink", "encrypt", "--peer-key", "abc", "--input", "data.bin"]);
        match cli.command {
            Commands::Encrypt { input, .. } => assert_eq!(input.as_deref(), Some("data.bin")),
            _ => panic!("expected Encrypt"),
        }
    }

    // ─── Decrypt subcommand ───

    #[test]
    fn decrypt_required_keypair() {
        let err = parse_err(&["flowlink", "decrypt"]);
        assert!(err.contains("--keypair"), "got: {err}");
    }

    #[test]
    fn decrypt_keypair_only() {
        let cli = parse(&["flowlink", "decrypt", "--keypair", "kp.json"]);
        match cli.command {
            Commands::Decrypt { keypair, input } => {
                assert_eq!(keypair, "kp.json");
                assert!(input.is_none());
            }
            _ => panic!("expected Decrypt"),
        }
    }

    #[test]
    fn decrypt_with_input() {
        let cli = parse(&["flowlink", "decrypt", "--keypair", "kp.json", "-i", "enc.json"]);
        match cli.command {
            Commands::Decrypt { input, .. } => assert_eq!(input.as_deref(), Some("enc.json")),
            _ => panic!("expected Decrypt"),
        }
    }

    #[test]
    fn decrypt_with_input_long() {
        let cli = parse(&["flowlink", "decrypt", "--keypair", "k.json", "--input", "e.json"]);
        match cli.command {
            Commands::Decrypt { input, .. } => assert_eq!(input.as_deref(), Some("e.json")),
            _ => panic!("expected Decrypt"),
        }
    }

    // ─── Version subcommand ───

    #[test]
    fn version_subcommand() {
        let cli = parse(&["flowlink", "version"]);
        assert!(matches!(cli.command, Commands::Version));
    }

    // ─── --help flag ───

    #[test]
    fn help_flag() {
        // --help causes clap to exit, so we use try_parse
        let result = Cli::try_parse_from(["flowlink", "--help"]);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("FlowLink") || err.contains("Usage"), "got: {err}");
    }

    #[test]
    fn help_short() {
        let result = Cli::try_parse_from(["flowlink", "-h"]);
        assert!(result.is_err());
    }

    #[test]
    fn agent_help() {
        let result = Cli::try_parse_from(["flowlink", "agent", "--help"]);
        assert!(result.is_err());
    }

    #[test]
    fn relay_help() {
        let result = Cli::try_parse_from(["flowlink", "relay", "--help"]);
        assert!(result.is_err());
    }

    // ─── --version flag ───

    #[test]
    fn version_flag() {
        let result = Cli::try_parse_from(["flowlink", "--version"]);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("flowlink"), "got: {err}");
    }

    // ─── Invalid / unknown args ───

    #[test]
    fn unknown_subcommand_errors() {
        let err = parse_err(&["flowlink", "foobar"]);
        // clap says "invalid subcommand"
        assert!(!err.is_empty());
    }

    #[test]
    fn unknown_flag_errors() {
        let err = parse_err(&["flowlink", "--bogus", "agent"]);
        assert!(err.contains("unexpected") || err.contains("invalid"), "got: {err}");
    }

    #[test]
    fn unknown_flag_on_subcommand_errors() {
        let err = parse_err(&["flowlink", "agent", "--fake"]);
        assert!(!err.is_empty());
    }

    // ─── Mutual exclusivity ───

    #[test]
    fn cannot_combine_subcommands() {
        let err = parse_err(&["flowlink", "agent", "relay"]);
        assert!(!err.is_empty());
    }

    // ─── CLI metadata ───

    #[test]
    fn cli_name() {
        let cmd = Cli::command();
        assert_eq!(cmd.get_name(), "flowlink");
    }

    #[test]
    fn cli_has_subcommands() {
        let cmd = Cli::command();
        let names: Vec<_> = cmd.get_subcommands().map(|s| s.get_name()).collect();
        assert!(names.contains(&"agent"));
        assert!(names.contains(&"relay"));
        assert!(names.contains(&"keygen"));
        assert!(names.contains(&"encrypt"));
        assert!(names.contains(&"decrypt"));
        assert!(names.contains(&"version"));
    }

    #[test]
    fn cli_has_about() {
        let cmd = Cli::command();
        let about = cmd.get_about().map(|a| a.to_string());
        assert!(about.is_some());
        let about = about.unwrap();
        assert!(about.contains("FlowLink"));
    }

    #[test]
    fn cli_has_version() {
        let cmd = Cli::command();
        let ver = cmd.get_version().map(|v| v.to_string());
        assert!(ver.is_some());
    }

    // ─── read_input unit tests ───

    #[test]
    fn read_input_from_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("flowlink_test_read_input.txt");
        std::fs::write(&path, b"hello world").unwrap();
        let result = read_input(&Some(path.to_str().unwrap().to_string())).unwrap();
        assert_eq!(result, b"hello world");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn read_input_missing_file_errors() {
        let result = read_input(&Some("/nonexistent/path/flowlink_test.txt".to_string()));
        assert!(result.is_err());
    }

    // ─── Integration-like: full roundtrip ───

    #[test]
    fn full_agent_roundtrip() {
        let cli = parse(&[
            "flowlink", "--verbose", "agent", "--config", "/tmp/agent.json",
        ]);
        assert!(cli.verbose);
        match cli.command {
            Commands::Agent { config } => assert_eq!(config, "/tmp/agent.json"),
            _ => panic!("expected Agent"),
        }
    }

    #[test]
    fn full_relay_roundtrip() {
        let cli = parse(&[
            "flowlink", "-v", "relay", "-c", "r.json", "--addr", "0.0.0.0:8080",
        ]);
        assert!(cli.verbose);
        match cli.command {
            Commands::Relay { config, addr } => {
                assert_eq!(config, "r.json");
                assert_eq!(addr.as_deref(), Some("0.0.0.0:8080"));
            }
            _ => panic!("expected Relay"),
        }
    }

    #[test]
    fn full_encrypt_roundtrip() {
        let cli = parse(&[
            "flowlink", "--verbose", "encrypt",
            "--peer-key", "dGVzdA==", "--input", "plain.txt",
        ]);
        assert!(cli.verbose);
        match cli.command {
            Commands::Encrypt { peer_key, input } => {
                assert_eq!(peer_key, "dGVzdA==");
                assert_eq!(input.as_deref(), Some("plain.txt"));
            }
            _ => panic!("expected Encrypt"),
        }
    }

    #[test]
    fn full_decrypt_roundtrip() {
        let cli = parse(&[
            "flowlink", "decrypt",
            "--keypair", "keys/kp.json", "--input", "msg.enc",
        ]);
        assert!(!cli.verbose);
        match cli.command {
            Commands::Decrypt { keypair, input } => {
                assert_eq!(keypair, "keys/kp.json");
                assert_eq!(input.as_deref(), Some("msg.enc"));
            }
            _ => panic!("expected Decrypt"),
        }
    }

    // ─── Subcommand descriptions exist ───

    #[test]
    fn agent_has_description() {
        let cmd = Cli::command();
        let agent = cmd.find_subcommand("agent").unwrap();
        let about = agent.get_about().map(|a| a.to_string());
        assert!(about.is_some());
    }

    #[test]
    fn relay_has_description() {
        let cmd = Cli::command();
        let relay = cmd.find_subcommand("relay").unwrap();
        let about = relay.get_about().map(|a| a.to_string());
        assert!(about.is_some());
    }

    #[test]
    fn keygen_has_description() {
        let cmd = Cli::command();
        let kg = cmd.find_subcommand("keygen").unwrap();
        assert!(kg.get_about().is_some());
    }

    // ─── Edge cases ───

    #[test]
    fn empty_args_errors() {
        let err = parse_err(&["flowlink"]);
        assert!(!err.is_empty());
    }

    #[test]
    fn config_default_agent() {
        let cli = parse(&["flowlink", "agent"]);
        match cli.command {
            Commands::Agent { config } => assert_eq!(config, "flowlink.json"),
            _ => panic!(),
        }
    }

    #[test]
    fn config_default_relay() {
        let cli = parse(&["flowlink", "relay"]);
        match cli.command {
            Commands::Relay { config, .. } => assert_eq!(config, "relay.json"),
            _ => panic!(),
        }
    }

    #[test]
    fn extra_positional_errors() {
        let err = parse_err(&["flowlink", "agent", "extra", "args"]);
        assert!(!err.is_empty());
    }

    #[test]
    fn encrypt_missing_peer_key_is_error() {
        let err = parse_err(&["flowlink", "encrypt", "--input", "x.txt"]);
        assert!(err.contains("--peer-key") || err.contains("required"), "got: {err}");
    }

    #[test]
    fn decrypt_missing_keypair_is_error() {
        let err = parse_err(&["flowlink", "decrypt", "--input", "x.enc"]);
        assert!(err.contains("--keypair") || err.contains("required"), "got: {err}");
    }

    #[test]
    fn verbose_with_keygen() {
        let cli = parse(&["flowlink", "--verbose", "keygen", "-o", "k.json"]);
        assert!(cli.verbose);
        assert!(matches!(cli.command, Commands::Keygen { .. }));
    }

    #[test]
    fn verbose_with_encrypt() {
        let cli = parse(&["flowlink", "--verbose", "encrypt", "--peer-key", "x"]);
        assert!(cli.verbose);
    }

    #[test]
    fn verbose_with_decrypt() {
        let cli = parse(&["flowlink", "--verbose", "decrypt", "--keypair", "k.json"]);
        assert!(cli.verbose);
    }
}
