// FlowLink Shield — Level 3: Interpreter heuristics

use super::level2::bash_ast;
use super::types::{bn, Threat};

pub fn check_level3(binary: &str, args: &[String]) -> Option<Threat> {
    let b = bn(binary);
    match b {
        "python3" | "python" => l3_lang(
            args,
            "-c",
            &[
                "shutil.rmtree",
                "os.system",
                "os.popen",
                "subprocess.call",
                "subprocess.run",
                "subprocess.Popen",
                "os.remove",
                "os.unlink",
            ],
            "Python",
        ),
        "node" => l3_lang(
            args,
            "-e",
            &[
                "child_process.exec",
                "child_process.spawn",
                "fs.rm",
                "fs.rmSync",
                "fs.rmdir",
                "fs.unlink",
                "process.exit",
            ],
            "Node",
        ),
        "perl" => l3_lang(
            args,
            "-e",
            &["system(", "exec(", "unlink(", "rmdir(", "`rm"],
            "Perl",
        ),
        "ruby" => l3_lang(
            args,
            "-e",
            &[
                "system(",
                "exec(",
                "FileUtils.rm_rf",
                "FileUtils.rm_r",
                "File.delete",
            ],
            "Ruby",
        ),
        "php" => l3_lang(
            args,
            "-r",
            &[
                "system(",
                "exec(",
                "passthru(",
                "shell_exec(",
                "unlink(",
                "rmdir(",
            ],
            "PHP",
        ),
        "ansible" | "ansible-playbook" => l3_ansible(args),
        "kubectl" => l3_kubectl(args),
        "crontab" if args.iter().any(|a| a == "-r") && !args.iter().any(|a| a == "-l") => Some(
            Threat::warn("crontab", "Crontab Remove", "Removing all cron jobs".into()),
        ),
        _ => None,
    }
}

fn l3_lang(args: &[String], flag: &str, patterns: &[&str], lang: &str) -> Option<Threat> {
    let code = inline_code(args, flag)?;
    for p in patterns {
        if code.contains(p) {
            return Some(Threat::critical(
                "lang_exec",
                &format!("{}: {}", lang, p),
                format!("{} dangerous call", lang),
            ));
        }
    }
    None
}

fn inline_code<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == flag {
            return it.next().map(|s| s.as_str());
        }
        if a.starts_with(flag) && a.len() > flag.len() {
            return Some(&a[flag.len()..]);
        }
    }
    None
}

fn l3_ansible(args: &[String]) -> Option<Threat> {
    if args.iter().any(|a| a == "-m") && args.iter().any(|a| a == "shell" || a == "command") {
        if let Some(pos) = args.iter().position(|a| a == "-a") {
            if let Some(cmd) = args.get(pos + 1) {
                return bash_ast(cmd);
            }
        }
    }
    None
}

fn l3_kubectl(args: &[String]) -> Option<Threat> {
    if args.iter().any(|a| a == "exec") {
        if let Some(pos) = args.iter().position(|a| a == "--") {
            return bash_ast(&args[pos + 1..].join(" "));
        }
    }
    if args.iter().any(|a| a == "delete")
        && args
            .iter()
            .any(|a| a == "--force" || a == "--grace-period=0")
    {
        return Some(Threat::warn(
            "k8s_force",
            "kubectl Force Delete",
            "Force deleting K8s resource".into(),
        ));
    }
    None
}
