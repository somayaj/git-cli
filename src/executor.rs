use colored::Colorize;
use regex::Regex;
use std::collections::HashMap;
use std::process::Command;

pub struct ParsedOutput {
    pub lines: Vec<OutputLine>,
}

pub enum OutputLine {
    Comment(String),
    GitCommand(String),
    Other(String),
}

const DESTRUCTIVE_PATTERNS: &[&str] = &[
    "push --force",
    "push -f ",
    "reset --hard",
    "clean -f",
    "clean -df",
    "clean -fd",
    "clean -xf",
    "branch -D ",
];

pub fn parse_response(response: &str) -> ParsedOutput {
    let cleaned = sanitize_response(response);

    let lines = cleaned
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                OutputLine::Comment(trimmed.to_string())
            } else if trimmed.starts_with("git ") || trimmed.starts_with("gh ") {
                if is_safe_command(trimmed) {
                    OutputLine::GitCommand(trimmed.to_string())
                } else {
                    OutputLine::Other(format!("[BLOCKED] {trimmed}"))
                }
            } else {
                OutputLine::Other(trimmed.to_string())
            }
        })
        .collect();

    ParsedOutput { lines }
}

fn sanitize_response(response: &str) -> String {
    let mut result = response.to_string();

    result = result.replace("```bash", "");
    result = result.replace("```shell", "");
    result = result.replace("```sh", "");
    result = result.replace("```", "");

    let lines: Vec<String> = result
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            if let Some(rest) = strip_numbering(trimmed) {
                rest.to_string()
            } else {
                trimmed.to_string()
            }
        })
        .collect();

    let joined = join_multiline_commands(&lines).join("\n");
    fix_case_globs(&joined)
}

fn fix_case_globs(cmd: &str) -> String {
    if let Ok(re) = Regex::new(r"([0-9a-f]{7,40})\)") {
        re.replace_all(cmd, "${1}*)").to_string()
    } else {
        cmd.to_string()
    }
}

fn join_multiline_commands(lines: &[String]) -> Vec<String> {
    let mut merged: Vec<String> = Vec::new();
    let mut accumulator = String::new();
    let mut open_single = false;
    let mut open_double = false;

    for line in lines {
        if accumulator.is_empty() {
            if line.trim().starts_with('#') || line.trim().is_empty() {
                merged.push(line.clone());
                continue;
            }
            accumulator = line.clone();
        } else {
            accumulator.push(' ');
            accumulator.push_str(line.trim());
        }

        open_single = false;
        open_double = false;
        for ch in accumulator.chars() {
            match ch {
                '\'' if !open_double => open_single = !open_single,
                '"' if !open_single => open_double = !open_double,
                _ => {}
            }
        }

        if !open_single && !open_double {
            merged.push(accumulator.clone());
            accumulator.clear();
        }
    }

    if !accumulator.is_empty() {
        merged.push(accumulator);
    }

    merged
}

fn strip_numbering(line: &str) -> Option<&str> {
    let bytes = line.as_bytes();
    let mut i = 0;

    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 {
        return None;
    }

    if i + 1 < bytes.len() && (bytes[i] == b'.' || bytes[i] == b')' || bytes[i] == b':') {
        let rest = &line[i + 1..];
        return Some(rest.trim_start());
    }

    let lower = line.to_lowercase();
    if lower.starts_with("step ") {
        if let Some(colon_pos) = line.find(':') {
            return Some(line[colon_pos + 1..].trim_start());
        }
    }

    None
}

fn is_safe_command(cmd: &str) -> bool {
    if !cmd.starts_with("git ") && !cmd.starts_with("gh ") {
        return false;
    }

    if cmd.starts_with("gh ") {
        return true;
    }

    // Check for injection patterns only OUTSIDE of quotes
    let unquoted = strip_quoted_sections(cmd);
    let injection_patterns = ["&&", "||", ";", "$(", "`", "|"];
    for pat in &injection_patterns {
        if unquoted.contains(pat) {
            return false;
        }
    }

    if let Some(n) = extract_head_offset(cmd) {
        let commit_count = get_commit_count();
        if n > commit_count {
            eprintln!(
                "  {} HEAD~{} but repo only has {} commit(s). Skipping.",
                "Warning:".yellow().bold(),
                n,
                commit_count
            );
            return false;
        }
    }

    if cmd.contains("git push") && cmd.contains(':') {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if let Some(refspec) = parts.last() {
            if refspec.contains(':') && !refspec.contains("refs/tags/") {
                eprintln!(
                    "  {} Blocked push with refspec `{}`. Use `git push origin <branch>` and `gh pr create` instead.",
                    "Warning:".yellow().bold(),
                    refspec
                );
                return false;
            }
        }
    }

    if cmd.contains("rebase -i") || cmd.contains("rebase --interactive") {
        eprintln!(
            "  {} Blocked `rebase -i` (no interactive editor available). Use `git reset --soft` or `git filter-branch`.",
            "Warning:".yellow().bold(),
        );
        return false;
    }

    // Block commit with trailing bare hash references (LLM hallucination)
    if cmd.contains("git commit") {
        if let Ok(re) = Regex::new(r"[0-9a-f]{7,}\^?\s*$") {
            let after_message = if let Some(pos) = cmd.find("-m ") {
                let rest = &cmd[pos + 3..];
                // Skip past the quoted message
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
                if !trailing.is_empty() && re.is_match(trailing) {
                    eprintln!(
                        "  {} Malformed commit command with trailing hash. Skipping.",
                        "Warning:".yellow().bold(),
                    );
                    return false;
                }
            }
        }
    }

    true
}

fn strip_quoted_sections(cmd: &str) -> String {
    let mut result = String::new();
    let mut in_single = false;
    let mut in_double = false;

    for ch in cmd.chars() {
        match ch {
            '\'' if !in_double => {
                in_single = !in_single;
            }
            '"' if !in_single => {
                in_double = !in_double;
            }
            _ if !in_single && !in_double => {
                result.push(ch);
            }
            _ => {}
        }
    }
    result
}

fn extract_head_offset(cmd: &str) -> Option<u32> {
    Regex::new(r"HEAD~(\d+)")
        .ok()?
        .captures(cmd)
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

fn shell_split(cmd: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    for ch in cmd.chars() {
        match ch {
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
            }
            ' ' if !in_single_quote && !in_double_quote => {
                if !current.is_empty() {
                    parts.push(current.clone());
                    current.clear();
                }
            }
            _ => {
                current.push(ch);
            }
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }

    parts
}

pub fn has_destructive_commands(parsed: &ParsedOutput) -> bool {
    parsed.lines.iter().any(|line| {
        if let OutputLine::GitCommand(cmd) = line {
            DESTRUCTIVE_PATTERNS.iter().any(|p| cmd.contains(p))
        } else {
            false
        }
    })
}

pub fn display(parsed: &ParsedOutput) {
    println!();
    for line in &parsed.lines {
        match line {
            OutputLine::Comment(c) => println!("  {}", c.dimmed()),
            OutputLine::GitCommand(cmd) => {
                if DESTRUCTIVE_PATTERNS.iter().any(|p| cmd.contains(p)) {
                    println!("  {} {}", "⚠".yellow(), cmd.red().bold());
                } else {
                    println!("  {}", cmd.green().bold());
                }
            }
            OutputLine::Other(text) => println!("  {}", text.yellow()),
        }
    }
    println!();
}

pub fn execute_commands(parsed: &ParsedOutput, force: bool) -> Result<(), String> {
    let commands: Vec<&str> = parsed
        .lines
        .iter()
        .filter_map(|l| match l {
            OutputLine::GitCommand(cmd) => Some(cmd.as_str()),
            _ => None,
        })
        .collect();

    if commands.is_empty() {
        println!("{}", "No git commands found to execute.".yellow());
        return Ok(());
    }

    if !force && has_destructive_commands(parsed) {
        eprintln!(
            "  {} Contains destructive commands. Use {} to override.",
            "Blocked:".red().bold(),
            "--force".bold()
        );
        return Ok(());
    }

    let has_creates = commands.iter().any(|c| c.starts_with("gh pr create"));
    let has_merges = commands.iter().any(|c| c.starts_with("gh pr merge"));

    let mut pr_number_map: HashMap<u32, u32> = HashMap::new();
    let mut created_prs: Vec<u32> = Vec::new();

    let predicted_merge_numbers: Vec<u32> = if has_creates && has_merges {
        let open_prs = get_open_pr_numbers();
        commands
            .iter()
            .filter_map(|c| extract_pr_merge_number(c))
            .filter(|n| !open_prs.contains(n))
            .collect()
    } else {
        Vec::new()
    };

    let mut failed_cmds: Vec<String> = Vec::new();

    for cmd_str in commands {
        let actual_cmd = if cmd_str.starts_with("gh pr merge") {
            if let Some(n) = extract_pr_merge_number(cmd_str) {
                if let Some(&actual) = pr_number_map.get(&n) {
                    let replaced = cmd_str.replacen(&n.to_string(), &actual.to_string(), 1);
                    eprintln!(
                        "  {} PR #{} → #{} (actual)",
                        "Remapped:".yellow().bold(),
                        n,
                        actual
                    );
                    replaced
                } else {
                    cmd_str.to_string()
                }
            } else {
                cmd_str.to_string()
            }
        } else {
            cmd_str.to_string()
        };

        println!("  {} {}", "Running:".cyan().bold(), actual_cmd);

        let parts = shell_split(&actual_cmd);
        if parts.is_empty() {
            continue;
        }

        let output = Command::new(&parts[0])
            .args(&parts[1..])
            .output()
            .map_err(|e| format!("Failed to run `{actual_cmd}`: {e}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !stdout.trim().is_empty() {
            println!("{stdout}");
        }
        if !stderr.trim().is_empty() {
            eprintln!("{stderr}");
        }

        if !output.status.success() {
            let is_gh_merge = actual_cmd.starts_with("gh pr merge");
            let is_gh_create = actual_cmd.starts_with("gh pr create");

            if is_gh_merge {
                let stderr_str = stderr.to_string();
                if stderr_str.contains("not allowed") || stderr_str.contains("not mergeable") {
                    if retry_merge_with_fallback(&actual_cmd).is_some() {
                        continue;
                    }
                }
                eprintln!(
                    "  {} `{}` failed (exit code {}). Continuing with remaining commands...",
                    "Skipped:".yellow().bold(),
                    actual_cmd,
                    output.status.code().unwrap_or(-1)
                );
                failed_cmds.push(actual_cmd);
                continue;
            }

            if is_gh_create {
                eprintln!(
                    "  {} `{}` failed (exit code {}). Continuing with remaining commands...",
                    "Skipped:".yellow().bold(),
                    actual_cmd,
                    output.status.code().unwrap_or(-1)
                );
                failed_cmds.push(actual_cmd);
                continue;
            }

            let is_push_to_existing = actual_cmd.starts_with("git push")
                && (stderr.contains("non-fast-forward") || stderr.contains("already exists"));
            if is_push_to_existing {
                eprintln!(
                    "  {} Push failed but branch likely exists on remote. Continuing...",
                    "Note:".yellow().bold(),
                );
                continue;
            }

            return Err(format!(
                "Command `{actual_cmd}` failed with exit code {}",
                output.status.code().unwrap_or(-1)
            ));
        }

        if cmd_str.starts_with("gh pr create") {
            if let Some(pr_num) = parse_pr_number_from_output(&stdout) {
                let idx = created_prs.len();
                created_prs.push(pr_num);
                if let Some(&predicted) = predicted_merge_numbers.get(idx) {
                    pr_number_map.insert(predicted, pr_num);
                }
            }
        }
    }

    if failed_cmds.is_empty() {
        println!("  {}", "All commands completed successfully.".green().bold());
    } else {
        eprintln!();
        eprintln!(
            "  {} {} command(s) failed:",
            "Summary:".yellow().bold(),
            failed_cmds.len()
        );
        for cmd in &failed_cmds {
            eprintln!("    {} {}", "✗".red(), cmd);
        }
        eprintln!();
        return Err(format!("{} command(s) failed (see above)", failed_cmds.len()));
    }

    Ok(())
}

fn retry_merge_with_fallback(original_cmd: &str) -> Option<()> {
    let strategies = ["--squash", "--rebase"];
    for strategy in &strategies {
        let retry_cmd = original_cmd
            .replace("--merge", strategy);
        eprintln!(
            "  {} Retrying with `{}`...",
            "Fallback:".cyan().bold(),
            strategy
        );
        let parts = shell_split(&retry_cmd);
        if parts.is_empty() {
            continue;
        }
        let output = Command::new(&parts[0])
            .args(&parts[1..])
            .output()
            .ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stdout.trim().is_empty() {
            println!("{stdout}");
        }
        if !stderr.trim().is_empty() {
            eprintln!("{stderr}");
        }
        if output.status.success() {
            eprintln!(
                "  {} Merged successfully with `{}`",
                "OK:".green().bold(),
                strategy
            );
            return Some(());
        }
    }
    None
}

fn extract_pr_merge_number(cmd: &str) -> Option<u32> {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.len() >= 4 && parts[0] == "gh" && parts[1] == "pr" && parts[2] == "merge" {
        parts[3].parse().ok()
    } else {
        None
    }
}

fn parse_pr_number_from_output(output: &str) -> Option<u32> {
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.contains("/pull/") {
            return trimmed.rsplit('/').next()?.parse().ok();
        }
    }
    None
}

fn get_open_pr_numbers() -> Vec<u32> {
    Command::new("gh")
        .args([
            "pr", "list", "--state", "open", "--json", "number",
            "--template", "{{range .}}{{.number}}\n{{end}}",
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter_map(|l| l.trim().parse().ok())
                .collect()
        })
        .unwrap_or_default()
}
