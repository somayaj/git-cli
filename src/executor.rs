use colored::Colorize;
use regex::Regex;
use std::process::Command;

pub use crate::command::{
    quotes_balanced, shell_split, strip_quoted_sections, try_parse_command, BlockReason,
    extract_bad_flag, extract_head_branch, extract_pr_merge_number, fix_gh_pr_create_head,
    parse_pr_number_from_output, remove_flag, ParsedCommand, QuoteAwareChars,
};

pub struct ParsedOutput {
    pub lines: Vec<OutputLine>,
}

pub enum OutputLine {
    Comment(String),
    Command(ParsedCommand),
    /// Legacy fallback when the command passes `is_safe_command` but has no typed parser yet.
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
        .map(|line| classify_line(line, &cleaned))
        .collect();

    ParsedOutput { lines }
}

pub fn classify_line(line: &str, full_response: &str) -> OutputLine {
    let trimmed = line.trim();
    if trimmed.starts_with('#') {
        OutputLine::Comment(trimmed.to_string())
    } else if trimmed.starts_with("git ") || trimmed.starts_with("gh ") {
        let sanitized = strip_inline_comment(&strip_pipe_suffix(trimmed));
        if has_placeholder(&sanitized) {
            OutputLine::Other(format!("[BLOCKED placeholder] {trimmed}"))
        } else if is_cherry_pick_in_pr_context(&sanitized, full_response) {
            OutputLine::Other(format!("[BLOCKED cherry-pick] {trimmed} — use `gh pr create` with different --base instead"))
        } else if is_checkout_for_cherry_pick(&sanitized, full_response) {
            OutputLine::Other(format!("[BLOCKED checkout] {trimmed} — not needed for PR workflow"))
        } else {
            match try_parse_command(&sanitized) {
                Ok(action) => OutputLine::Command(ParsedCommand::new(sanitized, action)),
                Err(_) if is_safe_command(&sanitized) => OutputLine::GitCommand(sanitized),
                Err(reason) => {
                    let msg = if reason != BlockReason::UnknownSubcommand {
                        format!("[BLOCKED: {}] {trimmed}", reason.as_blocked_label())
                    } else {
                        format!("[BLOCKED] {trimmed}")
                    };
                    OutputLine::Other(msg)
                }
            }
        }
    } else {
        OutputLine::Other(trimmed.to_string())
    }
}

pub fn sanitize_response(response: &str) -> String {
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

pub fn fix_case_globs(cmd: &str) -> String {
    if let Ok(re) = Regex::new(r"([0-9a-f]{7,40})\)") {
        re.replace_all(cmd, "${1}*)").to_string()
    } else {
        cmd.to_string()
    }
}

pub fn join_multiline_commands(lines: &[String]) -> Vec<String> {
    let mut merged: Vec<String> = Vec::new();
    let mut accumulator = String::new();

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

        if quotes_balanced(&accumulator) {
            merged.push(accumulator.clone());
            accumulator.clear();
        }
    }

    if !accumulator.is_empty() {
        merged.push(accumulator);
    }

    merged
}

pub fn strip_numbering(line: &str) -> Option<&str> {
    let digit_end = line
        .char_indices()
        .take_while(|(_, c)| c.is_ascii_digit())
        .last()
        .map(|(i, c)| i + c.len_utf8())?;

    let rest = &line[digit_end..];
    if rest.starts_with('.') || rest.starts_with(')') || rest.starts_with(':') {
        return Some(rest[1..].trim_start());
    }

    if line.to_lowercase().starts_with("step ") {
        return line.find(':').map(|pos| line[pos + 1..].trim_start());
    }

    None
}

pub fn is_cherry_pick_in_pr_context(cmd: &str, full_response: &str) -> bool {
    if !cmd.contains("cherry-pick") {
        return false;
    }
    let lower = full_response.to_lowercase();
    lower.contains("gh pr create") || lower.contains("gh pr merge")
}

pub fn is_checkout_for_cherry_pick(cmd: &str, full_response: &str) -> bool {
    if !cmd.starts_with("git checkout ") {
        return false;
    }
    let lower = full_response.to_lowercase();
    lower.contains("cherry-pick") && (lower.contains("gh pr create") || lower.contains("gh pr merge"))
}

pub fn strip_inline_comment(cmd: &str) -> String {
    if let Some(pos) = find_unquoted_hash(cmd) {
        cmd[..pos].trim().to_string()
    } else {
        cmd.to_string()
    }
}

pub fn find_unquoted_hash(cmd: &str) -> Option<usize> {
    let mut prev_char: Option<char> = None;

    for (byte_idx, ch, quoted) in QuoteAwareChars::new(cmd) {
        if !quoted && ch == '#' && prev_char == Some(' ') {
            let next_char = cmd[byte_idx + ch.len_utf8()..].chars().next();
            if !next_char.map_or(false, |c| c.is_ascii_digit()) {
                return Some(byte_idx);
            }
        }
        prev_char = Some(ch);
    }
    None
}

pub fn has_placeholder(cmd: &str) -> bool {
    let unquoted = strip_quoted_sections(cmd);
    unquoted.contains('<') && unquoted.contains('>')
}

pub fn strip_pipe_suffix(cmd: &str) -> String {
    let unquoted = strip_quoted_sections(cmd);
    if unquoted.contains('|') {
        let original_pos = find_unquoted_pipe(cmd);
        if let Some(pos) = original_pos {
            let stripped = cmd[..pos].trim().to_string();
            let pipe_part = cmd[pos..].trim();
            eprintln!(
                "  {} Stripped `{}` (pipes not supported)",
                "Note:".yellow().bold(),
                pipe_part
            );
            return stripped;
        }
    }
    cmd.to_string()
}

pub fn find_unquoted_pipe(cmd: &str) -> Option<usize> {
    QuoteAwareChars::new(cmd)
        .find(|&(_, ch, quoted)| ch == '|' && !quoted)
        .map(|(i, _, _)| i)
}

pub fn is_safe_command(cmd: &str) -> bool {
    if !cmd.starts_with("git ") && !cmd.starts_with("gh ") {
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

    if cmd.starts_with("gh ") {
        return cmd.starts_with("gh pr create")
            || cmd.starts_with("gh pr merge")
            || cmd.starts_with("gh pr view")
            || cmd.starts_with("gh pr list");
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
            if refspec.starts_with(':') {
                // `:branch` is valid delete syntax — allow it
            } else if refspec.contains(':') && !refspec.contains("refs/tags/") {
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

pub fn extract_head_offset(cmd: &str) -> Option<u32> {
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

pub fn has_destructive_commands(parsed: &ParsedOutput) -> bool {
    parsed.lines.iter().any(|line| match line {
        OutputLine::Command(c) if c.action.is_destructive() => true,
        OutputLine::GitCommand(cmd) => DESTRUCTIVE_PATTERNS.iter().any(|p| cmd.contains(p)),
        _ => false,
    })
}

pub fn display(parsed: &ParsedOutput) {
    println!();
    for line in &parsed.lines {
        match line {
            OutputLine::Comment(c) => println!("  {}", c.dimmed()),
            OutputLine::Command(c) => {
                if c.action.is_destructive() {
                    println!("  {} {}", "⚠".yellow(), c.raw.red().bold());
                } else {
                    println!("  {}", c.raw.green().bold());
                }
            }
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
    let executables = collect_executables(parsed);
    crate::command::run::execute_all(executables, force, has_destructive_commands(parsed))
}

fn collect_executables(parsed: &ParsedOutput) -> Vec<crate::command::Executable> {
    use crate::command::Executable;
    parsed
        .lines
        .iter()
        .filter_map(|line| match line {
            OutputLine::Command(c) => Some(Executable::Typed(c.action.clone())),
            OutputLine::GitCommand(s) => Some(Executable::Legacy(s.clone())),
            _ => None,
        })
        .collect()
}

