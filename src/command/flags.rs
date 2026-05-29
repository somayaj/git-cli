use super::quote::shell_split;

pub fn extract_bad_flag(stderr: &str) -> Option<String> {
    for line in stderr.lines() {
        let line = line.trim();
        if line.contains("unrecognized argument:") {
            return line
                .split("unrecognized argument:")
                .nth(1)
                .map(|s| s.trim().to_string());
        }
        if line.contains("unknown option") {
            if let Some(flag) = line.split("unknown option").nth(1) {
                let cleaned = flag
                    .trim()
                    .trim_start_matches(':')
                    .trim()
                    .trim_matches('\'')
                    .trim_matches('`')
                    .trim();
                if !cleaned.is_empty() {
                    return Some(format!("--{cleaned}"));
                }
            }
        }
        if line.contains("unknown switch") {
            if let Some(flag) = line.split('`').nth(1) {
                return Some(flag.trim_matches('\'').to_string());
            }
        }
        if line.contains("do not take a branch name") {
            return Some("__strip_trailing_arg__".to_string());
        }
    }
    None
}

pub fn remove_flag(cmd: &str, flag: &str) -> String {
    if flag == "__strip_trailing_arg__" {
        let parts = shell_split(cmd);
        if parts.len() > 1 {
            let without_last = &parts[..parts.len() - 1];
            return without_last
                .iter()
                .map(|p| {
                    if p.contains(' ') {
                        format!("\"{p}\"")
                    } else {
                        p.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");
        }
        return cmd.to_string();
    }
    let flag_with_space = format!(" {flag}");
    let result = cmd.replace(&flag_with_space, "");
    if result == cmd {
        cmd.replace(flag, "").replace("  ", " ")
    } else {
        result
    }
}

pub fn parse_pr_number_from_output(output: &str) -> Option<u32> {
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.contains("/pull/") {
            return trimmed.rsplit('/').next()?.parse().ok();
        }
    }
    None
}

pub fn extract_pr_merge_number(cmd: &str) -> Option<u32> {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.len() >= 4 && parts[0] == "gh" && parts[1] == "pr" && parts[2] == "merge" {
        parts[3].parse().ok()
    } else {
        None
    }
}

pub fn extract_head_branch(cmd: &str) -> Option<String> {
    let parts = shell_split(cmd);
    for (i, part) in parts.iter().enumerate() {
        if part == "--head" {
            return parts.get(i + 1).cloned();
        }
    }
    None
}

pub fn fix_gh_pr_create_head(cmd: &str) -> String {
    let Some(head) = extract_head_branch(cmd) else {
        return cmd.to_string();
    };
    if branch_exists(&head) {
        return cmd.to_string();
    }
    let Some(current) = current_branch() else {
        return cmd.to_string();
    };
    use colored::Colorize;
    eprintln!(
        "  {} Replacing hallucinated --head `{}` with current branch `{}`",
        "Auto:".cyan().bold(),
        head,
        current
    );
    cmd.replacen(
        &format!("--head {head}"),
        &format!("--head {current}"),
        1,
    )
}

fn branch_exists(name: &str) -> bool {
    std::process::Command::new("git")
        .args([
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{name}"),
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn current_branch() -> Option<String> {
    std::process::Command::new("git")
        .args(["branch", "--show-current"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}
