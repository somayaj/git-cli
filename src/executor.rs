use colored::Colorize;
use regex::Regex;
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
            } else if trimmed.starts_with("git ") {
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
    if !cmd.starts_with("git ") {
        return false;
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

    for cmd_str in commands {
        println!("  {} {}", "Running:".cyan().bold(), cmd_str);

        let parts = shell_split(cmd_str);
        if parts.is_empty() {
            continue;
        }

        let output = Command::new(&parts[0])
            .args(&parts[1..])
            .output()
            .map_err(|e| format!("Failed to run `{cmd_str}`: {e}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !stdout.trim().is_empty() {
            println!("{stdout}");
        }
        if !stderr.trim().is_empty() {
            eprintln!("{stderr}");
        }

        if !output.status.success() {
            return Err(format!(
                "Command `{cmd_str}` failed with exit code {}",
                output.status.code().unwrap_or(-1)
            ));
        }
    }

    println!("  {}", "All commands completed successfully.".green().bold());
    Ok(())
}
