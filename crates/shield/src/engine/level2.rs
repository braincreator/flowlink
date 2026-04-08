// FlowLink Shield — Level 2: AST analysis via tree-sitter-bash

use super::types::{Threat, bn};

pub fn check_level2(binary: &str, args: &[String], raw: &str) -> Option<Threat> {
    let b = bn(binary);
    if matches!(b, "bash" | "sh" | "zsh" | "dash" | "ksh" | "fish") {
        let script = extract_script(args)?;
        return bash_ast(script);
    }
    if raw.contains("| bash") || raw.contains("| sh ") || raw.contains("base64 -d |") {
        return bash_ast(raw);
    }
    if raw.starts_with("eval ") || raw.starts_with("exec ") {
        return bash_ast(raw);
    }
    None
}

fn extract_script<'a>(args: &'a [String]) -> Option<&'a str> {
    let mut it = args.iter();
    while let Some(a) = it.next() { if a == "-c" { return it.next().map(|s| s.as_str()); } }
    None
}

pub(super) fn bash_ast(script: &str) -> Option<Threat> {
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_bash::LANGUAGE.into()).ok()?;
    let tree = p.parse(script, None)?;
    walk_ast(tree.root_node(), script)
}

fn walk_ast(node: tree_sitter::Node, src: &str) -> Option<Threat> {
    if let Some(t) = check_ast(node, src) { return Some(t); }
    let mut c = node.walk();
    for ch in node.children(&mut c) { if let Some(t) = walk_ast(ch, src) { return Some(t); } }
    None
}

fn check_ast(node: tree_sitter::Node, src: &str) -> Option<Threat> {
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
        "eval" => { for a in &args { if let Some(t) = bash_ast(a) { return Some(t); } } }
        "exec" => { if let Some(t) = bash_ast(&args.join(" ")) { return Some(t); } }
        "git" => {
            // Catch git push --force, git reset --hard, git clean -fdx, git filter-branch inside bash -c
            if args.iter().any(|a| a == "push") && args.iter().any(|a| a == "--force" || a == "--force-with-lease" || a == "-f") {
                return Some(Threat::high("ast_git_push_f", "Git Force Push (AST)", "Force push inside script".into()));
            }
            if args.iter().any(|a| a == "reset") && args.iter().any(|a| a == "--hard" || a == "-H") {
                return Some(Threat::high("ast_git_reset_hard", "Git Reset --hard (AST)", "Hard reset inside script".into()));
            }
            if args.iter().any(|a| a == "clean") && args.iter().any(|a| a.contains("fdx") || (a == "-f" && args.iter().any(|b| b == "-d"))) {
                return Some(Threat::high("ast_git_clean", "Git Clean (AST)", "Clean with -fdx inside script".into()));
            }
            if args.iter().any(|a| a == "filter-branch" || a == "filter-repo") {
                return Some(Threat::critical("ast_git_filter", "Git Filter (AST)", "History rewrite inside script".into()));
            }
        }
        _ => {}
    }

    let text = node.utf8_text(src.as_bytes()).unwrap_or("");
    if has_obfuscation(text) {
        return Some(Threat::critical("ast_obf", "Obfuscated Cmd", "Obfuscated dangerous command".into()));
    }
    None
}

fn has_obfuscation(text: &str) -> bool {
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
