use super::parse::{BlockReason, GitAction, PlannedAction, ResetMode};
use regex::Regex;
use std::process::Command;

pub fn parse_git(parts: &[String], cmd: &str) -> Result<PlannedAction, BlockReason> {
    if parts.len() < 2 {
        return Err(BlockReason::UnknownSubcommand);
    }

    match parts[1].as_str() {
        "status" => Ok(PlannedAction::Git(GitAction::Status)),
        "log" if parts.len() == 2 || parts[2..].iter().all(is_safe_log_arg) => {
            Ok(PlannedAction::Git(GitAction::Log))
        }
        "add" => parse_git_add(&parts[2..]),
        "commit" => parse_git_commit(parts, cmd),
        "checkout" => parse_git_checkout(&parts[2..]),
        "push" => parse_git_push(&parts[2..]),
        "reset" => parse_git_reset(&parts[2..]),
        "branch" if parts.len() >= 3 && (parts[2] == "-D" || parts[2] == "-d") => {
            Ok(PlannedAction::Git(GitAction::BranchDelete {
                name: parts[3].clone(),
            }))
        }
        "clean" => parse_git_clean(&parts[2..]),
        _ => Err(BlockReason::UnknownSubcommand),
    }
}

fn is_safe_log_arg(arg: &String) -> bool {
    arg.starts_with('-') && !arg.contains(';')
}

fn parse_git_add(args: &[String]) -> Result<PlannedAction, BlockReason> {
    if args.is_empty() {
        return Err(BlockReason::UnknownSubcommand);
    }
    Ok(PlannedAction::Git(GitAction::Add {
        paths: args.to_vec(),
    }))
}

fn parse_git_commit(parts: &[String], cmd: &str) -> Result<PlannedAction, BlockReason> {
    let mut amend = false;
    let mut message: Option<String> = None;
    let mut i = 2;
    while i < parts.len() {
        match parts[i].as_str() {
            "--amend" => amend = true,
            "-m" | "--message" => {
                i += 1;
                if i < parts.len() {
                    message = Some(parts[i].clone());
                }
            }
            _ => {}
        }
        i += 1;
    }

    let message = message.ok_or(BlockReason::MissingRequiredFlag)?;

    if malformed_commit_trailing_hash(cmd) {
        return Err(BlockReason::MalformedCommit);
    }

    Ok(PlannedAction::Git(GitAction::Commit { message, amend }))
}

fn malformed_commit_trailing_hash(cmd: &str) -> bool {
    let Ok(re) = Regex::new(r"[0-9a-f]{7,}\^?\s*$") else {
        return false;
    };
    let after_message = if let Some(pos) = cmd.find("-m ") {
        let rest = &cmd[pos + 3..];
        if rest.starts_with('"') {
            rest[1..].find('"').map(|end| &rest[end + 2..])
        } else if rest.starts_with('\'') {
            rest[1..].find('\'').map(|end| &rest[end + 2..])
        } else {
            rest.split_whitespace().nth(1).map(|s| s)
        }
    } else {
        None
    };

    if let Some(trailing) = after_message {
        let trailing = trailing.trim();
        !trailing.is_empty() && re.is_match(trailing)
    } else {
        false
    }
}

fn parse_git_checkout(args: &[String]) -> Result<PlannedAction, BlockReason> {
    if args.is_empty() {
        return Err(BlockReason::UnknownSubcommand);
    }
    if args[0] == "-b" {
        if args.len() < 2 {
            return Err(BlockReason::MissingRequiredFlag);
        }
        return Ok(PlannedAction::Git(GitAction::Checkout {
            branch: args[1].clone(),
            create: true,
        }));
    }
    Ok(PlannedAction::Git(GitAction::Checkout {
        branch: args[0].clone(),
        create: false,
    }))
}

fn parse_git_push(args: &[String]) -> Result<PlannedAction, BlockReason> {
    let mut force = false;
    let mut rest: Vec<&String> = Vec::new();
    for arg in args {
        match arg.as_str() {
            "--force" | "-f" => force = true,
            _ => rest.push(arg),
        }
    }

    if rest.len() == 2 {
        let remote = rest[0].clone();
        let refspec = rest[1].clone();
        if refspec.starts_with(':') {
            return Ok(PlannedAction::Git(GitAction::PushDeleteRemote {
                remote,
                branch: refspec.trim_start_matches(':').to_string(),
            }));
        }
        if refspec.contains(':') && !refspec.contains("refs/tags/") {
            return Err(BlockReason::InvalidRefspec);
        }
        return Ok(PlannedAction::Git(GitAction::Push {
            remote,
            branch: refspec,
            force,
        }));
    }

    Err(BlockReason::UnknownSubcommand)
}

fn parse_git_reset(args: &[String]) -> Result<PlannedAction, BlockReason> {
    let mut mode = ResetMode::Mixed;
    let mut target = "HEAD".to_string();
    for arg in args {
        match arg.as_str() {
            "--soft" => mode = ResetMode::Soft,
            "--mixed" => mode = ResetMode::Mixed,
            "--hard" => mode = ResetMode::Hard,
            other if !other.starts_with('-') => target = other.to_string(),
            _ => {}
        }
    }

    if let Some(n) = extract_head_offset(&target) {
        let commit_count = get_commit_count();
        if n > commit_count {
            return Err(BlockReason::InvalidHeadOffset);
        }
    }

    Ok(PlannedAction::Git(GitAction::Reset { mode, target }))
}

fn parse_git_clean(args: &[String]) -> Result<PlannedAction, BlockReason> {
    let force = args.iter().any(|a| {
        let s = a.as_str();
        s == "-f" || s == "-df" || s == "-fd" || s == "-xf" || s.contains("-f")
    });
    Ok(PlannedAction::Git(GitAction::Clean {
        force,
        extra: args.to_vec(),
    }))
}

fn extract_head_offset(target: &str) -> Option<u32> {
    Regex::new(r"HEAD~(\d+)")
        .ok()?
        .captures(target)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse().ok())
}

fn get_commit_count() -> u32 {
    Command::new("git")
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok())
        .unwrap_or(0)
}
