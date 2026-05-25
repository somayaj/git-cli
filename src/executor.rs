use colored::Colorize;
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

    // Strip markdown code fences
    result = result.replace("```bash", "");
    result = result.replace("```shell", "");
    result = result.replace("```sh", "");
    result = result.replace("```", "");

    // Strip numbered prefixes like "1. " or "Step 1: "
    let lines: Vec<String> = result
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            // "1. git ..." or "1) git ..."
            if let Some(rest) = strip_numbering(trimmed) {
                rest.to_string()
            } else {
                trimmed.to_string()
            }
        })
        .collect();

    lines.join("\n")
}

fn strip_numbering(line: &str) -> Option<&str> {
    let bytes = line.as_bytes();
    let mut i = 0;

    // Skip digits
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 {
        return None;
    }

    // Expect ". " or ") " or ": "
    if i + 1 < bytes.len() && (bytes[i] == b'.' || bytes[i] == b')' || bytes[i] == b':') {
        let rest = &line[i + 1..];
        return Some(rest.trim_start());
    }

    // "Step N: " pattern
    let lower = line.to_lowercase();
    if lower.starts_with("step ") {
        if let Some(colon_pos) = line.find(':') {
            return Some(line[colon_pos + 1..].trim_start());
        }
    }

    None
}

fn is_safe_command(cmd: &str) -> bool {
    // Block shell injection patterns
    let injection_patterns = ["&&", "||", ";", "$(", "`", "|"];
    for pat in &injection_patterns {
        if cmd.contains(pat) {
            return false;
        }
    }
    true
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

        let parts: Vec<&str> = cmd_str.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        let output = Command::new(parts[0])
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
