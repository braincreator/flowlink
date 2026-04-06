mod engine;
mod interceptor;
mod snapshot;
mod audit;
mod notifier;

use engine::{AnalysisEngine, Command, ThreatLevel};
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use snapshot::SnapshotBackend;
use audit::AuditLog;
use notifier::Notifier;

#[derive(Parser)]
#[command(name = "flowlink-shield", version)]
struct Args {
    #[arg(short, long)] webhook: Option<String>,
    #[arg(short, long, default_value = "./shield-audit.jsonl")] audit_log: PathBuf,
    #[arg(short, long)] snapshot_dataset: Option<String>,
    #[arg(long)] simulate: bool,
    #[arg(long)] dry_run: bool,
    #[arg(long)] no_ast: bool,
    #[arg(long)] no_interpreter: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let engine = AnalysisEngine { enable_ast: !args.no_ast, enable_interpreter: !args.no_interpreter };
    let snapshot_backend = SnapshotBackend::detect();
    let audit = Arc::new(RwLock::new(AuditLog::open(&args.audit_log)?));
    let notifier = Notifier::new(args.webhook.clone());

    if !args.simulate {
        println!("⚡ Production requires Linux kernel ≥ 5.4 + eBPF");
        println!("   Use --simulate for testing");
        return Ok(());
    }

    println!("🛡️ FlowLink Shield v0.1.0 — Simulation");
    println!("   3 levels: ARGS → AST (tree-sitter) → INTERPRETER");
    println!("   Type commands to test. 'quit' to exit.\n");

    let stdin = std::io::stdin();
    let mut line = String::new();
    loop {
        line.clear();
        if stdin.read_line(&mut line).is_err() { break; }
        let cmd = line.trim();
        if cmd.is_empty() || cmd == "quit" || cmd == "exit" { break; }

        let parts: Vec<&str> = cmd.split_whitespace().collect();
        let shield_cmd = Command {
            binary: parts.first().unwrap_or(&"").to_string(),
            args: parts.iter().map(|s| s.to_string()).collect(),
            raw: cmd.to_string(),
        };

        let result = engine.analyze(&shield_cmd);
        if result.safe {
            println!("✅ ALLOW");
        } else if let Some(ref t) = result.threat {
            let lvl = match result.level_used { 1 => "L1", 2 => "L2", 3 => "L3", _ => "?" };
            println!("{} [{}] → {} | {} | snap={} timeout={}s", t.level, lvl, t.name, t.description, t.snapshot, t.timeout_secs);
        }
    }

    Ok(())
}
