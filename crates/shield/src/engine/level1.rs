// FlowLink Shield — Level 1: Structured dangerous command detection

use super::types::{bn, Threat};

pub fn check_level1(binary: &str, args: &[String]) -> Option<Threat> {
    let b = bn(binary);
    match b {
        "rm" => l1_rm(args),
        _ if b.starts_with("mkfs") => Some(Threat::critical(
            "format_disk",
            "Disk Format",
            format!("Formatting: {}", args.join(" ")),
        )),
        "dd" => l1_dd(args),
        "shred" if args.iter().any(|a| a.starts_with('/')) => Some(Threat::critical(
            "shred",
            "File Shred",
            "Secure deleting files".into(),
        )),
        "docker" => l1_docker(args),
        "shutdown" | "poweroff" | "halt" | "reboot" => Some(Threat::critical(
            "shutdown",
            "System Shutdown",
            format!("Power: {}", b),
        )),
        "init" if args.iter().any(|a| a == "0" || a == "6") => Some(Threat::critical(
            "init_rl",
            "Init Runlevel",
            "Shutdown/reboot runlevel".into(),
        )),
        "systemctl" => l1_systemctl(args),
        "killall" | "pkill" => l1_killall(args),
        "chmod" => l1_chmod(args),
        "iptables" | "ip6tables" | "nft" => l1_fw(b, args),
        "mysql" | "psql" | "sqlite3" | "mongosh" | "redis-cli" => l1_db(b, args),
        "git" => l1_git(args),
        _ => None,
    }
}

fn l1_rm(args: &[String]) -> Option<Threat> {
    let (mut r, mut f, mut dp, mut np) = (false, false, false, false);
    for a in args.iter() {
        match a.as_str() {
            "--no-preserve-root" => np = true,
            "--recursive" => r = true,
            "--force" => f = true,
            s if s.starts_with('-') && !s.starts_with("--") => {
                let fl = s.trim_start_matches('-');
                if fl.contains('r') || fl.contains('R') {
                    r = true;
                }
                if fl.contains('f') {
                    f = true;
                }
            }
            s if !s.starts_with('-') => {
                for d in &[
                    "/", "/var", "/etc", "/usr", "/home", "/opt", "/srv", "/boot", "/root", "/sys",
                    "/proc", "/dev",
                ] {
                    if s == *d || (s.starts_with(d) && d.len() > 1) {
                        dp = true;
                    }
                }
                if s == "/*" || s == "/" || s == "*" {
                    dp = true;
                }
            }
            _ => {}
        }
    }
    if np {
        return Some(Threat::critical(
            "rm_npr",
            "rm --no-preserve-root",
            "Delete entire filesystem".into(),
        ));
    }
    if r && f && dp {
        return Some(Threat::critical(
            "rm_rf",
            "rm -rf",
            "Recursive force delete".into(),
        ));
    }
    if r && dp {
        return Some(Threat::warn(
            "rm_r",
            "Recursive rm",
            "Recursive delete on system path".into(),
        ));
    }
    None
}

fn l1_dd(args: &[String]) -> Option<Threat> {
    let s = args.join(" ");
    if s.contains("of=/dev/sd") || s.contains("of=/dev/nvme") || s.contains("of=/dev/vd") {
        return Some(Threat::critical(
            "dd_dev",
            "dd to Device",
            "Raw write to block device".into(),
        ));
    }
    None
}

fn l1_docker(args: &[String]) -> Option<Threat> {
    if args.len() < 2 {
        return None;
    }
    let has_f = || args.iter().any(|a| a == "-f" || a == "--force");
    match args[1].as_str() {
        "rm" if has_f() => Some(Threat::high(
            "docker_rm_f",
            "Docker Force RM",
            "Force removing container".into(),
        )),
        "rmi" if has_f() => Some(Threat::warn(
            "docker_rmi",
            "Docker Force RMI",
            "Force removing image".into(),
        )),
        "system"
            if args.iter().any(|a| a == "prune")
                && args.iter().any(|a| a == "-a" || a == "--all") =>
        {
            Some(Threat::high(
                "docker_prune",
                "Docker Prune All",
                "Pruning everything".into(),
            ))
        }
        "volume" if args.iter().any(|a| a == "rm" || a == "prune") => Some(Threat::warn(
            "docker_vol",
            "Docker Volume RM",
            "Removing volumes".into(),
        )),
        _ => None,
    }
}

fn l1_systemctl(args: &[String]) -> Option<Threat> {
    if args.len() < 3 {
        return None;
    }
    if !matches!(args[1].as_str(), "stop" | "disable" | "mask") {
        return None;
    }
    let crit = [
        "sshd",
        "ssh",
        "docker",
        "nginx",
        "postgresql",
        "mysql",
        "redis-server",
        "mongod",
        "firewalld",
    ];
    let rest = args[2..].join(" ");
    for &s in &crit {
        if rest.contains(s) {
            return Some(Threat::high(
                "svc_stop",
                "Stop Service",
                format!("Stopping: {}", s),
            ));
        }
    }
    None
}

fn l1_killall(args: &[String]) -> Option<Threat> {
    let crit = [
        "sshd",
        "systemd",
        "init",
        "docker",
        "nginx",
        "postgres",
        "mysql",
        "mongod",
        "redis-server",
        "kubelet",
    ];
    for a in args.iter() {
        if crit.contains(&a.as_str()) {
            return Some(Threat::high(
                "kill_crit",
                "Kill Critical",
                format!("Killing: {}", a),
            ));
        }
    }
    None
}

fn l1_chmod(args: &[String]) -> Option<Threat> {
    let r = args.iter().any(|a| a == "-R" || a == "--recursive");
    let w = args.iter().any(|a| a == "777" || a == "a+rwx");
    if r && w {
        return Some(Threat::warn(
            "chmod_777",
            "chmod 777 -R",
            "World-writable recursively".into(),
        ));
    }
    None
}

fn l1_fw(b: &str, args: &[String]) -> Option<Threat> {
    match b {
        "iptables" | "ip6tables"
            if args
                .iter()
                .any(|a| a == "-F" || a == "--flush" || a == "-X") =>
        {
            Some(Threat::high(
                "fw_flush",
                "Firewall Flush",
                "Flushing firewall rules".into(),
            ))
        }
        "nft" if args.iter().any(|a| a == "flush") && args.iter().any(|a| a == "ruleset") => Some(
            Threat::high("nft_flush", "nft Flush", "Flushing nftables".into()),
        ),
        _ => None,
    }
}

fn l1_db(b: &str, args: &[String]) -> Option<Threat> {
    let s = args.join(" ").to_uppercase();
    if s.contains("DROP DATABASE") || s.contains("DROP TABLE") || s.contains("DROP SCHEMA") {
        return Some(Threat::critical(
            "sql_drop",
            "SQL DROP",
            format!("Via {}", b),
        ));
    }
    if args.iter().any(|a| a == "-e" || a == "-c")
        && (s.contains("TRUNCATE") || s.contains("DELETE FROM"))
    {
        return Some(Threat::critical(
            "sql_destruct",
            "Destructive SQL",
            format!("Via {}", b),
        ));
    }
    None
}

fn l1_git(args: &[String]) -> Option<Threat> {
    if args.len() < 2 {
        return None;
    }
    // Check if any arg is an exact match or contains the char in a combined short-flag group.
    // e.g. has_flag(args, "-f") matches "-f", "--force", and "-fdx" (contains 'f').
    let has_flag = |flag: &str| -> bool {
        args.iter().any(|a| {
            if a == flag {
                return true;
            }
            // For short flags like "-f", also match combined: "-fd", "-fdx"
            if flag.starts_with('-')
                && flag.len() == 2
                && a.starts_with('-')
                && a.len() > 2
                && !a.starts_with("--")
            {
                a.chars().skip(1).any(|c| flag.contains(c))
            } else {
                false
            }
        })
    };
    let has_force = || {
        has_flag("--force") || has_flag("--force-with-lease") || has_flag("-f") || has_flag("-F")
    };
    match args[1].as_str() {
        // ── push --force / --force-with-lease ──
        "push" if has_force() => Some(Threat::high(
            "git_push_f",
            "Git Force Push",
            "Force pushing overwrites remote history".into(),
        )),
        // ── reset --hard ──
        "reset" if has_flag("--hard") || has_flag("-H") => {
            // skip binary + subcommand, find the target commit
            let to = args
                .iter()
                .skip(2)
                .find(|a| !a.starts_with('-'))
                .cloned()
                .unwrap_or_default();
            if to == "HEAD~0" || to.is_empty() {
                Some(Threat::high(
                    "git_reset_hard_head",
                    "Git Reset --hard HEAD",
                    "Resetting to HEAD discards all working changes".into(),
                ))
            } else {
                Some(Threat::warn(
                    "git_reset_hard",
                    "Git Reset --hard",
                    format!("Hard reset to {}", to),
                ))
            }
        }
        // ── clean -fd / -fdx ──
        "clean" => {
            let f = has_flag("-f");
            let d = has_flag("-d");
            let x = has_flag("-x") || has_flag("-X");
            if f && d && x {
                Some(Threat::high(
                    "git_clean_fdx",
                    "Git Clean -fdx",
                    "Removing ALL untracked files including ignored".into(),
                ))
            } else if f && d {
                Some(Threat::warn(
                    "git_clean_fd",
                    "Git Clean -fd",
                    "Removing untracked files and directories".into(),
                ))
            } else {
                None
            }
        }
        // ── branch -D (force delete) ──
        "branch" if has_flag("-D") => Some(Threat::warn(
            "git_branch_d",
            "Git Branch -D",
            "Force deleting branch without merge check".into(),
        )),
        // ── tag -d / -fd (delete tag) ──
        "tag" if has_flag("-d") => {
            let has_f = has_flag("-f");
            if has_f {
                Some(Threat::warn(
                    "git_tag_fd",
                    "Git Tag -fd",
                    "Force deleting tag".into(),
                ))
            } else {
                Some(Threat::warn(
                    "git_tag_d",
                    "Git Tag -d",
                    "Deleting tag".into(),
                ))
            }
        }
        // ── filter-branch / filter-repo ──
        "filter-branch" => Some(Threat::critical(
            "git_filter_branch",
            "Git Filter-Branch",
            "Rewriting entire repository history".into(),
        )),
        "filter-repo" => Some(Threat::critical(
            "git_filter_repo",
            "Git Filter-Repo",
            "Rewriting repository history (irreversible)".into(),
        )),
        // ── reflog expire + gc prune (history wipe) ──
        "reflog"
            if args.iter().any(|a| {
                a == "expire" || a == "--expire=now" || a == "--expire-unreachable=now"
            }) =>
        {
            Some(Threat::warn(
                "git_reflog_expire",
                "Git Reflog Expire",
                "Expiring reflog entries — history may be lost".into(),
            ))
        }
        "gc" if args
            .iter()
            .any(|a| a == "--prune=now" || a == "--aggressive") =>
        {
            Some(Threat::warn(
                "git_gc_prune",
                "Git GC Prune",
                "Pruning unreachable objects".into(),
            ))
        }
        // ── stash drop / clear ──
        "stash" if args.iter().any(|a| a == "drop" || a == "clear") => Some(Threat::warn(
            "git_stash_drop",
            "Git Stash Drop",
            "Dropping stash entries".into(),
        )),
        // ── revert --no-commit (partial revert, messy state) ──
        "revert" if args.iter().any(|a| a == "--no-commit" || a == "-n") => Some(Threat::warn(
            "git_revert_no_commit",
            "Git Revert No-Commit",
            "Reverting without auto-commit — staged changes need manual resolution".into(),
        )),
        // ── worktree remove / prune ──
        "worktree" if args.iter().any(|a| a == "remove" || a == "prune") => Some(Threat::warn(
            "git_worktree_rm",
            "Git Worktree Remove",
            "Removing working tree".into(),
        )),
        // ── submodule deinit ──
        "submodule"
            if args.iter().any(|a| a == "deinit")
                && args.iter().any(|a| a == "-f" || a == "--force") =>
        {
            Some(Threat::warn(
                "git_submodule_deinit",
                "Git Submodule Deinit",
                "Force deinitializing submodule".into(),
            ))
        }
        _ => None,
    }
}
