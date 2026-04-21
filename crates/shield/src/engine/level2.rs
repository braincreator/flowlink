// FlowLink Shield — Level 2: AST analysis via tree-sitter-bash

use super::types::{bn, Threat};

pub fn check_level2(binary: &str, args: &[String], raw: &str) -> Option<Threat> {
    // L1.5: Raw string patterns (pipe chains, network, cloud, SQL)
    if let Some(t) = check_raw_patterns(raw) {
        return Some(t);
    }

    let b = bn(binary);
    if matches!(b, "bash" | "sh" | "zsh" | "dash" | "ksh" | "fish") {
        let script = extract_script(args)?;
        return bash_ast(script);
    }
    if raw.contains("| bash") || raw.contains("| sh ") || raw.contains("base64 -d |") || raw.contains("base64 --decode |") || raw.contains("base64 -di") || raw.contains("|/bin/sh") || raw.contains("|/bin/bash") {
        return bash_ast(raw);
    }
    if raw.starts_with("eval ") || raw.starts_with("exec ") {
        return bash_ast(raw);
    }
    None
}

fn extract_script<'a>(args: &'a [String]) -> Option<&'a str> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "-c" {
            return it.next().map(|s| s.as_str());
        }
    }
    None
}

pub(super) fn bash_ast(script: &str) -> Option<Threat> {
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_bash::LANGUAGE.into()).ok()?;
    let tree = p.parse(script, None)?;
    walk_ast(tree.root_node(), script)
}

fn walk_ast(node: tree_sitter::Node, src: &str) -> Option<Threat> {
    if let Some(t) = check_ast(node, src) {
        return Some(t);
    }
    let mut c = node.walk();
    for ch in node.children(&mut c) {
        if let Some(t) = walk_ast(ch, src) {
            return Some(t);
        }
    }
    None
}

fn check_ast(node: tree_sitter::Node, src: &str) -> Option<Threat> {
    if node.kind() != "command" {
        return None;
    }
    let name = node
        .child_by_field_name("name")
        .and_then(|n| n.utf8_text(src.as_bytes()).ok())
        .unwrap_or("")
        .to_string();
    let args: Vec<String> = (0..node.child_count())
        .filter_map(|i| {
            let c = node.child(i)?;
            if matches!(
                c.kind(),
                "word" | "string" | "raw_string" | "ansii_c_string"
            ) {
                c.utf8_text(src.as_bytes())
                    .ok()
                    .map(|s| s.trim_matches('"').trim_matches('\'').to_string())
            } else {
                None
            }
        })
        .collect();

    match name.as_str() {
        "rm" => {
            let rf = args.iter().any(|a| {
                let a = a.trim_start_matches('-');
                a.contains('r') && a.contains('f')
            });
            let root = args.iter().any(|a| a.starts_with('/') || a == "*");
            if rf && root {
                return Some(Threat::critical(
                    "ast_rm",
                    "rm -rf (AST)",
                    "AST: destructive rm".into(),
                ));
            }
        }
        "eval" => {
            for a in &args {
                if let Some(t) = bash_ast(a) {
                    return Some(t);
                }
            }
        }
        "exec" => {
            if let Some(t) = bash_ast(&args.join(" ")) {
                return Some(t);
            }
        }
        "git" => {
            // Catch git push --force, git reset --hard, git clean -fdx, git filter-branch inside bash -c
            if args.iter().any(|a| a == "push")
                && args
                    .iter()
                    .any(|a| a == "--force" || a == "--force-with-lease" || a == "-f")
            {
                return Some(Threat::high(
                    "ast_git_push_f",
                    "Git Force Push (AST)",
                    "Force push inside script".into(),
                ));
            }
            if args.iter().any(|a| a == "reset") && args.iter().any(|a| a == "--hard" || a == "-H")
            {
                return Some(Threat::high(
                    "ast_git_reset_hard",
                    "Git Reset --hard (AST)",
                    "Hard reset inside script".into(),
                ));
            }
            if args.iter().any(|a| a == "clean")
                && args
                    .iter()
                    .any(|a| a.contains("fdx") || (a == "-f" && args.iter().any(|b| b == "-d")))
            {
                return Some(Threat::high(
                    "ast_git_clean",
                    "Git Clean (AST)",
                    "Clean with -fdx inside script".into(),
                ));
            }
            if args
                .iter()
                .any(|a| a == "filter-branch" || a == "filter-repo")
            {
                return Some(Threat::critical(
                    "ast_git_filter",
                    "Git Filter (AST)",
                    "History rewrite inside script".into(),
                ));
            }
        }
        _ => {}
    }

    let text = node.utf8_text(src.as_bytes()).unwrap_or("");
    if has_obfuscation(text) {
        return Some(Threat::critical(
            "ast_obf",
            "Obfuscated Cmd",
            "Obfuscated dangerous command".into(),
        ));
    }
    None
}

fn has_obfuscation(text: &str) -> bool {
    // Hex escape: $'\x72\x6d'
    let re = regex::Regex::new(r"\\x([0-9a-fA-F]{2})").ok();
    if let Some(re) = re {
        let dec: String = re
            .captures_iter(text)
            .filter_map(|c| u8::from_str_radix(c.get(1)?.as_str(), 16).ok())
            .map(|b| b as char)
            .collect();
        for kw in &["rm -rf", "mkfs", "dd if", "DROP", "shutdown", "/etc/shadow", "/etc/passwd"] {
            if dec.contains(kw) {
                return true;
            }
        }
    }
    // Octal escape: $'\162\155'
    let oct_re = regex::Regex::new(r"\\([0-7]{3})").ok();
    if let Some(re) = oct_re {
        let dec: String = re
            .captures_iter(text)
            .filter_map(|c| u8::from_str_radix(c.get(1)?.as_str(), 8).ok())
            .map(|b| b as char)
            .collect();
        for kw in &["rm -rf", "mkfs", "dd if", "DROP", "shutdown", "/etc/shadow"] {
            if dec.contains(kw) {
                return true;
            }
        }
    }
    if text.contains("base64") && (text.contains("| bash") || text.contains("| sh") || text.contains("|/bin/") || text.contains("-d |") || text.contains("--decode |")) {
        return true;
    }
    false
}

/// L1.5: Raw string pattern matching for constructs that bypass structured args.
/// Catches pipe-to-interpreter, download+execute, network listeners, destructive SQL,
/// system path redirections, fork bombs, and cloud CLI data operations.
fn check_raw_patterns(raw: &str) -> Option<Threat> {
    let lower = raw.to_lowercase();

    // ── Pipe to shell interpreters ── CRITICAL
    let shell_pipe_targets: &[(&str, &str)] = &[
        ("| bash", "bash"),
        ("| sh", "sh"),
        ("| zsh", "zsh"),
        ("| dash", "dash"),
        ("| ksh", "ksh"),
        ("| fish", "fish"),
        ("| python", "python"),
        ("| python3", "python3"),
        ("| perl", "perl"),
        ("| ruby", "ruby"),
        ("| node", "node"),
        ("| php", "php"),
    ];
    for (pattern, interpreter) in shell_pipe_targets {
        if lower.contains(pattern) {
            return Some(
                Threat::critical(
                    "pipe_to_interpreter",
                    "Pipe to Shell Interpreter",
                    format!("Command output piped to {} — enables arbitrary code execution from untrusted source", interpreter),
                )
                .with_suggestion("Download file first, review contents, then execute manually"),
            );
        }
    }

    // ── Data exfiltration via pipe to network ── HIGH
    // cat file | curl -X POST -d @-  OR  cat file | curl --data @-
    if (lower.contains("cat ") || lower.contains("cat\t")) && lower.contains("curl") && lower.contains("|" ) {
        return Some(
            Threat::high(
                "data_exfil",
                "Data Exfiltration",
                "File contents piped to curl — potential data exfiltration to external server".into(),
            )
            .with_suggestion("Review the destination URL. Use environment variables or secrets manager instead of files"),
        );
    }
    // cat /etc/shadow, cat .env, cat */secret*
    if lower.starts_with("cat ") {
        let sensitive = ["/etc/shadow", "/etc/passwd", ".env", "secret", "credential", "private_key", "id_rsa", "id_ed25519"];
        for s in &sensitive {
            if lower.contains(s) {
                return Some(
                    Threat::high(
                        "sensitive_file_read",
                        "Sensitive File Access",
                        format!("Reading sensitive file ({}) — credentials may be exposed", s),
                    )
                    .with_suggestion("Use environment variables or secrets manager for sensitive data"),
                );
            }
        }
    }

    // ── Download + execute patterns ── CRITICAL
    if (lower.starts_with("curl") || lower.starts_with("wget")) && lower.contains('|') {
        return Some(
            Threat::critical(
                "download_and_execute",
                "Download & Execute",
                "Downloads content and pipes it to another command — potential remote code execution".into(),
            )
            .with_suggestion("Download file first, review contents, then execute if safe"),
        );
    }

    // ── Network listener / reverse shell ── HIGH
    let net_listeners = ["nc -l", "ncat", "socat tcp-listen", "socat tcp-l"];
    for p in &net_listeners {
        if lower.contains(p) {
            return Some(
                Threat::high(
                    "network_listener",
                    "Network Listener",
                    "Network listener detected — potential reverse shell or data exfiltration".into(),
                )
                .with_suggestion("Ensure this is intentional. Consider using SSH tunnels instead"),
            );
        }
    }

    // ── SSH reverse tunnel ── HIGH
    // ssh -R creates a reverse tunnel (lowercase: -r)
    // Match: ssh with -R/-r flag (but not -L local forward)
    {
        let args: Vec<&str> = lower.split_whitespace().collect();
        let is_ssh = args.first().map(|a| *a == "ssh").unwrap_or(false);
        let has_reverse_flag = args.iter().any(|a| *a == "-r" || (a.starts_with("-r") && *a != "-r"));
        let has_local_forward = args.iter().any(|a| *a == "-l" || *a == "-l");
        if is_ssh && has_reverse_flag && !has_local_forward {
            return Some(
                Threat::high(
                    "ssh_reverse_tunnel",
                    "SSH Reverse Tunnel",
                    "SSH reverse tunnel exposes local ports to remote server".into(),
                )
                .with_suggestion("Verify the remote host is trusted. Use -N to prevent remote command execution"),
            );
        }
    }

    // ── Chmod 777 ── MEDIUM
    if lower.contains("chmod") && lower.contains("777") {
        return Some(
            Threat::warn(
                "chmod_777",
                "chmod 777",
                "Grants full permissions to everyone — security risk".into(),
            )
            .with_suggestion("Use least-privilege permissions (750, 640)"),
        );
    }

    // ── SQL destructive operations ── CRITICAL
    let sql_patterns = ["drop table", "drop database", "truncate table", "truncate database"];
    for p in &sql_patterns {
        if lower.contains(p) {
            return Some(
                Threat::critical(
                    "sql_destructive",
                    "Destructive SQL",
                    format!("{} — irreversible data destruction", p.to_uppercase()),
                )
                .with_suggestion("Add WHERE clause or use a transaction with rollback"),
            );
        }
    }

    // ── Output redirection to system paths ── HIGH
    let sys_redirects: &[(&str, &str)] = &[
        ("> /etc/", "/etc"),
        (">> /etc/", "/etc"),
        ("> /var/", "/var"),
        (">> /var/", "/var"),
        ("> /boot/", "/boot"),
        (">> /boot/", "/boot"),
        ("> /usr/", "/usr"),
        (">> /usr/", "/usr"),
    ];
    for (pattern, path) in sys_redirects {
        if lower.contains(pattern) {
            return Some(
                Threat::high(
                    "redirect_to_system",
                    "Write to System Path",
                    format!("Output redirected to system directory {} — could corrupt system", path),
                ),
            );
        }
    }

    // ── Fork bomb ── CRITICAL
    let fb_lower = lower.replace(' ', "");
    if fb_lower.contains(":(){:|:&};:") || (lower.contains(":()") && lower.contains("|&")) {
        return Some(Threat::critical(
            "fork_bomb",
            "Fork Bomb",
            "Fork bomb — will exhaust system resources and crash".into(),
        ));
    }

    // ── Cloud CLI data operations ── MEDIUM
    let cloud_patterns: &[(&str, &str)] = &[
        ("aws s3 cp", "AWS S3 copy"),
        ("aws s3 sync", "AWS S3 sync"),
        ("aws s3 rm", "AWS S3 delete"),
        ("aws rds delete", "AWS RDS delete"),
        ("gcloud storage cp", "GCS copy"),
        ("az storage blob", "Azure Blob"),
    ];
    for (pattern, label) in cloud_patterns {
        if lower.contains(pattern) {
            return Some(
                Threat::warn(
                    "cloud_data_op",
                    "Cloud Data Operation",
                    format!("{} detected — verify source/destination", label),
                ),
            );
        }
    }

    None
}
