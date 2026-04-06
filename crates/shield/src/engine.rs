// FlowLink Shield — 3-level command analysis engine
// L1: Structured args | L2: AST (tree-sitter-bash) | L3: Interpreter heuristics

use serde::Serialize;
use std::fmt;

#[derive(Debug, Clone, Serialize)]
pub enum ThreatLevel { Critical, High, Medium, Low }

#[derive(Debug, Clone, Serialize)]
pub struct Threat {
    pub id: String, pub name: String, pub description: String,
    pub level: ThreatLevel, pub snapshot: bool, pub timeout_secs: u64,
}

macro_rules! threat {
    ($method:ident, $id:expr, $name:expr, $desc:expr) => {
        Threat::$method($id, $name, $desc.to_string())
    };
}

impl Threat {
    fn critical(id: &str, name: &str, desc: String) -> Self {
        Self { id: id.into(), name: name.into(), description: desc,
               level: ThreatLevel::Critical, snapshot: true, timeout_secs: 60 }
    }
    fn high(id: &str, name: &str, desc: String) -> Self {
        Self { id: id.into(), name: name.into(), description: desc,
               level: ThreatLevel::High, snapshot: false, timeout_secs: 60 }
    }
    fn warn(id: &str, name: &str, desc: String) -> Self {
        Self { id: id.into(), name: name.into(), description: desc,
               level: ThreatLevel::Medium, snapshot: false, timeout_secs: 0 }
    }
}

impl fmt::Display for ThreatLevel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Critical => write!(f, "🚫 BLOCK"),
            Self::High => write!(f, "⛔ BLOCK"),
            Self::Medium => write!(f, "⚠️ WARN"),
            Self::Low => write!(f, "📝 LOG"),
        }
    }
}

pub struct Command { pub binary: String, pub args: Vec<String>, pub raw: String }

pub struct AnalysisResult { pub threat: Option<Threat>, pub level_used: u8, pub safe: bool }

pub struct AnalysisEngine { pub enable_ast: bool, pub enable_interpreter: bool }

impl AnalysisEngine {
    pub fn analyze(&self, cmd: &Command) -> AnalysisResult {
        if let Some(t) = self.level1(&cmd.binary, &cmd.args) {
            return AnalysisResult { threat: Some(t), level_used: 1, safe: false };
        }
        if self.enable_ast {
            if let Some(t) = self.level2(&cmd.binary, &cmd.args, &cmd.raw) {
                return AnalysisResult { threat: Some(t), level_used: 2, safe: false };
            }
        }
        if self.enable_interpreter {
            if let Some(t) = self.level3(&cmd.binary, &cmd.args) {
                return AnalysisResult { threat: Some(t), level_used: 3, safe: false };
            }
        }
        AnalysisResult { threat: None, level_used: 0, safe: true }
    }

    fn bn(p: &str) -> &str { p.rsplit('/').next().unwrap_or(p) }

    // ═══════════════════════════════════════════
    // LEVEL 1
    // ═══════════════════════════════════════════

    fn level1(&self, binary: &str, args: &[String]) -> Option<Threat> {
        let b = Self::bn(binary);
        match b {
            "rm" => self.l1_rm(args),
            _ if b.starts_with("mkfs") =>
                Some(Threat::critical("format_disk", "Disk Format", format!("Formatting: {}", args.join(" ")))),
            "dd" => self.l1_dd(args),
            "shred" if args.iter().skip(1).any(|a| a.starts_with('/')) =>
                Some(Threat::critical("shred", "File Shred", "Secure deleting files".into())),
            "docker" => self.l1_docker(args),
            "shutdown" | "poweroff" | "halt" | "reboot" =>
                Some(Threat::critical("shutdown", "System Shutdown", format!("Power: {}", b))),
            "init" if args.iter().any(|a| a == "0" || a == "6") =>
                Some(Threat::critical("init_rl", "Init Runlevel", "Shutdown/reboot runlevel".into())),
            "systemctl" => self.l1_systemctl(args),
            "killall" | "pkill" => self.l1_killall(args),
            "chmod" => self.l1_chmod(args),
            "iptables" | "ip6tables" | "nft" => self.l1_fw(b, args),
            "mysql" | "psql" | "sqlite3" | "mongosh" | "redis-cli" => self.l1_db(b, args),
            _ => None,
        }
    }

    fn l1_rm(&self, args: &[String]) -> Option<Threat> {
        let (mut r, mut f, mut dp, mut np) = (false, false, false, false);
        for a in args.iter().skip(1) {
            match a.as_str() {
                "--no-preserve-root" => np = true,
                "--recursive" => r = true,
                "--force" => f = true,
                s if s.starts_with('-') && !s.starts_with("--") => {
                    let fl = s.trim_start_matches('-');
                    if fl.contains('r') || fl.contains('R') { r = true; }
                    if fl.contains('f') { f = true; }
                }
                s if !s.starts_with('-') => {
                    for d in &["/","/var","/etc","/usr","/home","/opt","/srv","/boot","/root","/sys","/proc","/dev"] {
                        if s == *d || (s.starts_with(d) && d.len() > 1) { dp = true; }
                    }
                    if s == "/*" || s == "/" || s == "*" { dp = true; }
                }
                _ => {}
            }
        }
        if np { return Some(Threat::critical("rm_npr", "rm --no-preserve-root", "Delete entire filesystem".into())); }
        if r && f && dp { return Some(Threat::critical("rm_rf", "rm -rf", "Recursive force delete".into())); }
        if r && dp { return Some(Threat::warn("rm_r", "Recursive rm", "Recursive delete on system path".into())); }
        None
    }

    fn l1_dd(&self, args: &[String]) -> Option<Threat> {
        let s = args.join(" ");
        if s.contains("of=/dev/sd") || s.contains("of=/dev/nvme") || s.contains("of=/dev/vd") {
            return Some(Threat::critical("dd_dev", "dd to Device", "Raw write to block device".into()));
        }
        None
    }

    fn l1_docker(&self, args: &[String]) -> Option<Threat> {
        if args.len() < 2 { return None; }
        let has_f = || args.iter().any(|a| a == "-f" || a == "--force");
        match args[1].as_str() {
            "rm" if has_f() => Some(Threat::high("docker_rm_f", "Docker Force RM", "Force removing container".into())),
            "rmi" if has_f() => Some(Threat::warn("docker_rmi", "Docker Force RMI", "Force removing image".into())),
            "system" if args.iter().any(|a| a == "prune") && args.iter().any(|a| a == "-a" || a == "--all") =>
                Some(Threat::high("docker_prune", "Docker Prune All", "Pruning everything".into())),
            "volume" if args.iter().any(|a| a == "rm" || a == "prune") =>
                Some(Threat::warn("docker_vol", "Docker Volume RM", "Removing volumes".into())),
            _ => None,
        }
    }

    fn l1_systemctl(&self, args: &[String]) -> Option<Threat> {
        if args.len() < 3 { return None; }
        if !matches!(args[1].as_str(), "stop" | "disable" | "mask") { return None; }
        let crit = ["sshd","ssh","docker","nginx","postgresql","mysql","redis-server","mongod","firewalld"];
        let rest = args[2..].join(" ");
        for &s in &crit { if rest.contains(s) { return Some(Threat::high("svc_stop", "Stop Service", format!("Stopping: {}", s))); } }
        None
    }

    fn l1_killall(&self, args: &[String]) -> Option<Threat> {
        let crit = ["sshd","systemd","init","docker","nginx","postgres","mysql","mongod","redis-server","kubelet"];
        for a in args.iter().skip(1) { if crit.contains(&a.as_str()) { return Some(Threat::high("kill_crit", "Kill Critical", format!("Killing: {}", a))); } }
        None
    }

    fn l1_chmod(&self, args: &[String]) -> Option<Threat> {
        let r = args.iter().any(|a| a == "-R" || a == "--recursive");
        let w = args.iter().any(|a| a == "777" || a == "a+rwx");
        if r && w { return Some(Threat::warn("chmod_777", "chmod 777 -R", "World-writable recursively".into())); }
        None
    }

    fn l1_fw(&self, b: &str, args: &[String]) -> Option<Threat> {
        match b {
            "iptables" | "ip6tables" if args.iter().any(|a| a == "-F" || a == "--flush" || a == "-X") =>
                Some(Threat::high("fw_flush", "Firewall Flush", "Flushing firewall rules".into())),
            "nft" if args.iter().any(|a| a == "flush") && args.iter().any(|a| a == "ruleset") =>
                Some(Threat::high("nft_flush", "nft Flush", "Flushing nftables".into())),
            _ => None,
        }
    }

    fn l1_db(&self, b: &str, args: &[String]) -> Option<Threat> {
        let s = args.join(" ").to_uppercase();
        if s.contains("DROP DATABASE") || s.contains("DROP TABLE") || s.contains("DROP SCHEMA") {
            return Some(Threat::critical("sql_drop", "SQL DROP", format!("Via {}", b)));
        }
        if args.iter().any(|a| a == "-e" || a == "-c") && (s.contains("TRUNCATE") || s.contains("DELETE FROM")) {
            return Some(Threat::critical("sql_destruct", "Destructive SQL", format!("Via {}", b)));
        }
        None
    }

    // ═══════════════════════════════════════════
    // LEVEL 2 — AST
    // ═══════════════════════════════════════════

    fn level2(&self, binary: &str, args: &[String], raw: &str) -> Option<Threat> {
        let b = Self::bn(binary);
        if matches!(b, "bash" | "sh" | "zsh" | "dash" | "ksh" | "fish") {
            let script = self.extract_script(args)?;
            return self.bash_ast(script);
        }
        if raw.contains("| bash") || raw.contains("| sh ") || raw.contains("base64 -d |") {
            return self.bash_ast(raw);
        }
        if raw.starts_with("eval ") || raw.starts_with("exec ") {
            return self.bash_ast(raw);
        }
        None
    }

    fn extract_script<'a>(&self, args: &'a [String]) -> Option<&'a str> {
        let mut it = args.iter();
        while let Some(a) = it.next() { if a == "-c" { return it.next().map(|s| s.as_str()); } }
        None
    }

    fn bash_ast(&self, script: &str) -> Option<Threat> {
        let mut p = tree_sitter::Parser::new();
        p.set_language(&tree_sitter_bash::LANGUAGE.into()).ok()?;
        let tree = p.parse(script, None)?;
        self.walk_ast(tree.root_node(), script)
    }

    fn walk_ast(&self, node: tree_sitter::Node, src: &str) -> Option<Threat> {
        if let Some(t) = self.check_ast(node, src) { return Some(t); }
        let mut c = node.walk();
        for ch in node.children(&mut c) { if let Some(t) = self.walk_ast(ch, src) { return Some(t); } }
        None
    }

    fn check_ast(&self, node: tree_sitter::Node, src: &str) -> Option<Threat> {
        if node.kind() != "command" { return None; }
        let name = node.child_by_field_name("name")
            .and_then(|n| n.utf8_text(src.as_bytes()).ok()).unwrap_or("").to_string();
        let args: Vec<String> = (0..node.child_count()).filter_map(|i| {
            let c = node.child(i)?;
            if matches!(c.kind(), "word" | "string" | "raw_string" | "ansii_c_string") {
                c.utf8_text(src.as_bytes()).ok().map(|s| s.trim_matches('"').trim_matches('\'').to_string())
            } else { None }
        }).collect();

        match name.as_str() {
            "rm" => {
                let rf = args.iter().any(|a| { let a = a.trim_start_matches('-'); a.contains('r') && a.contains('f') });
                let root = args.iter().any(|a| a.starts_with('/') || a == "*");
                if rf && root { return Some(Threat::critical("ast_rm", "rm -rf (AST)", "AST: destructive rm".into())); }
            }
            "eval" => { for a in &args { if let Some(t) = self.bash_ast(a) { return Some(t); } } }
            "exec" => { if let Some(t) = self.bash_ast(&args.join(" ")) { return Some(t); } }
            _ => {}
        }

        let text = node.utf8_text(src.as_bytes()).unwrap_or("");
        if self.has_obfuscation(text) {
            return Some(Threat::critical("ast_obf", "Obfuscated Cmd", "Obfuscated dangerous command".into()));
        }
        None
    }

    fn has_obfuscation(&self, text: &str) -> bool {
        let re = regex::Regex::new(r"\\x([0-9a-fA-F]{2})").ok();
        if let Some(re) = re {
            let dec: String = re.captures_iter(text)
                .filter_map(|c| u8::from_str_radix(c.get(1)?.as_str(), 16).ok())
                .map(|b| b as char).collect();
            for kw in &["rm -rf", "mkfs", "dd if", "DROP", "shutdown"] { if dec.contains(kw) { return true; } }
        }
        if text.contains("base64") && (text.contains("| bash") || text.contains("| sh")) { return true; }
        false
    }

    // ═══════════════════════════════════════════
    // LEVEL 3 — Interpreter
    // ═══════════════════════════════════════════

    fn level3(&self, binary: &str, args: &[String]) -> Option<Threat> {
        let b = Self::bn(binary);
        match b {
            "python3" | "python" => self.l3_lang(args, "-c", &["shutil.rmtree","os.system","os.popen","subprocess.call","subprocess.run","subprocess.Popen","os.remove","os.unlink"], "Python"),
            "node" => self.l3_lang(args, "-e", &["child_process.exec","child_process.spawn","fs.rm","fs.rmSync","fs.rmdir","fs.unlink","process.exit"], "Node"),
            "perl" => self.l3_lang(args, "-e", &["system(","exec(","unlink(","rmdir(","`rm"], "Perl"),
            "ruby" => self.l3_lang(args, "-e", &["system(","exec(","FileUtils.rm_rf","FileUtils.rm_r","File.delete"], "Ruby"),
            "php" => self.l3_lang(args, "-r", &["system(","exec(","passthru(","shell_exec(","unlink(","rmdir("], "PHP"),
            "ansible" | "ansible-playbook" => self.l3_ansible(args),
            "kubectl" => self.l3_kubectl(args),
            "crontab" if args.iter().any(|a| a == "-r") && !args.iter().any(|a| a == "-l") =>
                Some(Threat::warn("crontab", "Crontab Remove", "Removing all cron jobs".into())),
            _ => None,
        }
    }

    fn l3_lang(&self, args: &[String], flag: &str, patterns: &[&str], lang: &str) -> Option<Threat> {
        let code = self.inline_code(args, flag)?;
        for p in patterns {
            if code.contains(p) { return Some(Threat::critical("lang_exec", &format!("{}: {}", lang, p), format!("{} dangerous call", lang))); }
        }
        None
    }

    fn inline_code<'a>(&self, args: &'a [String], flag: &str) -> Option<&'a str> {
        let mut it = args.iter();
        while let Some(a) = it.next() {
            if a == flag { return it.next().map(|s| s.as_str()); }
            if a.starts_with(flag) && a.len() > flag.len() { return Some(&a[flag.len()..]); }
        }
        None
    }

    fn l3_ansible(&self, args: &[String]) -> Option<Threat> {
        if args.iter().any(|a| a == "-m") && args.iter().any(|a| a == "shell" || a == "command") {
            if let Some(pos) = args.iter().position(|a| a == "-a") {
                if let Some(cmd) = args.get(pos + 1) { return self.bash_ast(cmd); }
            }
        }
        None
    }

    fn l3_kubectl(&self, args: &[String]) -> Option<Threat> {
        if args.iter().any(|a| a == "exec") {
            if let Some(pos) = args.iter().position(|a| a == "--") {
                return self.bash_ast(&args[pos+1..].join(" "));
            }
        }
        if args.iter().any(|a| a == "delete") && args.iter().any(|a| a == "--force" || a == "--grace-period=0") {
            return Some(Threat::warn("k8s_force", "kubectl Force Delete", "Force deleting K8s resource".into()));
        }
        None
    }
}
