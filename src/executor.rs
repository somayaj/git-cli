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

pub struct QuoteAwareChars<'a> {
    inner: std::str::CharIndices<'a>,
    in_single: bool,
    in_double: bool,
}

impl<'a> QuoteAwareChars<'a> {
    pub fn new(s: &'a str) -> Self {
        Self {
            inner: s.char_indices(),
            in_single: false,
            in_double: false,
        }
    }
}

impl Iterator for QuoteAwareChars<'_> {
    type Item = (usize, char, bool);

    fn next(&mut self) -> Option<Self::Item> {
        let (i, ch) = self.inner.next()?;
        match ch {
            '\'' if !self.in_double => self.in_single = !self.in_single,
            '"' if !self.in_single => self.in_double = !self.in_double,
            _ => {}
        }
        let quoted = self.in_single || self.in_double;
        Some((i, ch, quoted))
    }
}

pub fn quotes_balanced(s: &str) -> bool {
    let mut qac = QuoteAwareChars::new(s);
    while qac.next().is_some() {}
    !qac.in_single && !qac.in_double
}

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
        } else if is_safe_command(&sanitized) {
            OutputLine::GitCommand(sanitized)
        } else {
            OutputLine::Other(format!("[BLOCKED] {trimmed}"))
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

pub fn strip_quoted_sections(cmd: &str) -> String {
    QuoteAwareChars::new(cmd)
        .filter(|&(_, ch, quoted)| !quoted && ch != '\'' && ch != '"')
        .map(|(_, ch, _)| ch)
        .collect()
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

pub fn shell_split(cmd: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut qac = QuoteAwareChars::new(cmd);

    while let Some((_, ch, _)) = qac.next() {
        let is_unquoted_space = ch == ' ' && !qac.in_single && !qac.in_double;
        let is_delimiter = (ch == '\'' && !qac.in_double) || (ch == '"' && !qac.in_single);

        if is_unquoted_space {
            if !current.is_empty() {
                parts.push(current.clone());
                current.clear();
            }
        } else if !is_delimiter {
            current.push(ch);
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

    if commands.iter().any(|c| c.starts_with("gh ")) && !crate::doctor::gh_on_path() {
        return Err(
            "GitHub CLI (gh) not found on PATH. Install: https://cli.github.com — then run `gh auth login`"
                .to_string(),
        );
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
    let mut branch_pushed = false;

    for cmd_str in commands {
        let mut actual_cmd = if cmd_str.starts_with("gh pr merge") {
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
            } else if !created_prs.is_empty() {
                // No PR number given — inject the last created PR number
                let last_pr = created_prs[created_prs.len() - 1];
                let fixed = cmd_str.replacen("gh pr merge", &format!("gh pr merge {}", last_pr), 1);
                eprintln!(
                    "  {} Injecting PR #{} (last created)",
                    "Auto:".cyan().bold(),
                    last_pr
                );
                fixed
            } else {
                cmd_str.to_string()
            }
        } else {
            cmd_str.to_string()
        };

        if actual_cmd.starts_with("gh pr create") {
            actual_cmd = fix_gh_pr_create_head(&actual_cmd);
        }

        if actual_cmd.starts_with("gh pr create") && !branch_pushed {
            if let Some(branch) = extract_head_branch(&actual_cmd) {
                eprintln!("  {} Pushing branch `{}` to remote first...", "Auto:".cyan().bold(), branch);
                let push_out = Command::new("git")
                    .args(["push", "origin", &branch])
                    .output();
                if let Ok(o) = &push_out {
                    let out = String::from_utf8_lossy(&o.stdout);
                    let err = String::from_utf8_lossy(&o.stderr);
                    if !out.trim().is_empty() { println!("{out}"); }
                    if !err.trim().is_empty() { eprintln!("{err}"); }
                }
                branch_pushed = true;
            }
        }

        println!("  {} {}", "Running:".cyan().bold(), actual_cmd);

        let parts = shell_split(&actual_cmd);
        if parts.is_empty() {
            continue;
        }

        let (output, actual_cmd) = run_with_flag_retry(&actual_cmd)?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !stdout.trim().is_empty() {
            println!("{stdout}");
        }
        if !stderr.trim().is_empty() {
            eprintln!("{stderr}");
        }

        if !output.status.success() {
            // git checkout -b fails when branch already exists → retry without -b
            if actual_cmd.starts_with("git checkout -b ")
                && (stderr.contains("already exists") || stderr.contains("already exist"))
            {
                let branch = actual_cmd.trim_start_matches("git checkout -b ").trim();
                eprintln!(
                    "  {} Branch already exists, switching to it instead...",
                    "Auto:".cyan().bold()
                );
                let retry = Command::new("git").args(["checkout", branch]).output();
                if let Ok(o) = retry {
                    let out = String::from_utf8_lossy(&o.stdout);
                    let err = String::from_utf8_lossy(&o.stderr);
                    if !out.trim().is_empty() { println!("{out}"); }
                    if !err.trim().is_empty() { eprintln!("{err}"); }
                    if o.status.success() {
                        continue;
                    }
                }
            }

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

            let is_branch_delete = actual_cmd.contains("branch -D") || actual_cmd.contains("branch -d");
            if is_branch_delete {
                if stderr.contains("checked out") || stderr.contains("Cannot delete") {
                    eprintln!("  {} Switching to main before deleting...", "Auto:".cyan().bold());
                    let _ = Command::new("git").args(["checkout", "main"]).output();
                    let retry = Command::new(&parts[0]).args(&parts[1..]).output();
                    if let Ok(o) = retry {
                        if o.status.success() {
                            let out = String::from_utf8_lossy(&o.stdout);
                            if !out.trim().is_empty() { println!("{out}"); }
                            continue;
                        }
                    }
                }
                eprintln!(
                    "  {} Branch may already be deleted. Continuing...",
                    "Note:".yellow().bold(),
                );
                continue;
            }

            let is_remote_delete = actual_cmd.contains("push origin --delete") || actual_cmd.contains("push origin :");
            if is_remote_delete {
                eprintln!(
                    "  {} Branch may already be deleted. Continuing...",
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

    if has_creates || has_merges {
        auto_merge_remaining_prs();
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

fn run_with_flag_retry(cmd: &str) -> Result<(std::process::Output, String), String> {
    let mut current_cmd = cmd.to_string();
    for _ in 0..3 {
        let parts = shell_split(&current_cmd);
        if parts.is_empty() {
            return Err("Empty command".to_string());
        }
        let output = Command::new(&parts[0])
            .args(&parts[1..])
            .output()
            .map_err(|e| format!("Failed to run `{current_cmd}`: {e}"))?;

        if output.status.success() {
            return Ok((output, current_cmd));
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        if let Some(bad_flag) = extract_bad_flag(&stderr) {
            eprintln!(
                "  {} Removing hallucinated flag `{}`",
                "Fix:".yellow().bold(),
                bad_flag
            );
            current_cmd = remove_flag(&current_cmd, &bad_flag);
            println!("  {} {}", "Retrying:".cyan().bold(), current_cmd);
        } else {
            return Ok((output, current_cmd));
        }
    }
    let parts = shell_split(&current_cmd);
    let output = Command::new(&parts[0])
        .args(&parts[1..])
        .output()
        .map_err(|e| format!("Failed to run `{current_cmd}`: {e}"))?;
    Ok((output, current_cmd))
}

pub fn extract_bad_flag(stderr: &str) -> Option<String> {
    for line in stderr.lines() {
        let line = line.trim();
        if line.contains("unrecognized argument:") {
            return line.split("unrecognized argument:").nth(1)
                .map(|s| s.trim().to_string());
        }
        if line.contains("unknown option") {
            if let Some(flag) = line.split("unknown option").nth(1) {
                let cleaned = flag.trim()
                    .trim_start_matches(':')
                    .trim()
                    .trim_matches('\'')
                    .trim_matches('`')
                    .trim();
                if !cleaned.is_empty() {
                    return Some(format!("--{}", cleaned));
                }
            }
        }
        if line.contains("unknown switch") {
            if let Some(flag) = line.split('`').nth(1) {
                return Some(flag.trim_matches('\'').to_string());
            }
        }
        // "do not take a branch name" → git branch -r/-a was given an extra arg
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
            return without_last.iter()
                .map(|p| if p.contains(' ') { format!("\"{}\"", p) } else { p.clone() })
                .collect::<Vec<_>>()
                .join(" ");
        }
        return cmd.to_string();
    }
    let flag_with_space = format!(" {}", flag);
    let result = cmd.replace(&flag_with_space, "");
    if result == cmd {
        cmd.replace(flag, "").replace("  ", " ")
    } else {
        result
    }
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

fn auto_merge_remaining_prs() {
    let current_branch = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

    let Some(branch) = current_branch else { return };

    let pr_output = Command::new("gh")
        .args([
            "pr", "list", "--state", "open", "--head", &branch,
            "--json", "number", "--template", "{{range .}}{{.number}}\n{{end}}",
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string());

    let Some(prs) = pr_output else { return };
    let pr_numbers: Vec<u32> = prs.lines().filter_map(|l| l.trim().parse().ok()).collect();

    if pr_numbers.is_empty() {
        return;
    }

    eprintln!(
        "\n  {} {} open PR(s) remaining for `{}`, merging...",
        "Auto-merge:".cyan().bold(),
        pr_numbers.len(),
        branch
    );

    for (i, pr) in pr_numbers.iter().enumerate() {
        let is_last = i == pr_numbers.len() - 1;
        let mut args = vec!["pr".to_string(), "merge".to_string(), pr.to_string()];
        args.push("--squash".to_string());
        if is_last {
            args.push("--delete-branch".to_string());
        }
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        eprintln!("  {} gh pr merge {} --squash{}", "Running:".cyan().bold(), pr,
            if is_last { " --delete-branch" } else { "" });

        let output = Command::new("gh").args(&arg_refs).output();
        if let Ok(o) = output {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let stderr = String::from_utf8_lossy(&o.stderr);
            if !stdout.trim().is_empty() { println!("{stdout}"); }
            if !stderr.trim().is_empty() { eprintln!("{stderr}"); }
            if !o.status.success() {
                eprintln!("  {} PR #{} merge failed, trying --rebase...", "Fallback:".yellow().bold(), pr);
                let pr_str = pr.to_string();
                let mut retry_args = vec!["pr", "merge", &pr_str, "--rebase"];
                if is_last { retry_args.push("--delete-branch"); }
                let _ = Command::new("gh").args(&retry_args).output();
            }
        }
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
    Command::new("git")
        .args(["show-ref", "--verify", "--quiet", &format!("refs/heads/{name}")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn current_branch() -> Option<String> {
    Command::new("git")
        .args(["branch", "--show-current"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn extract_pr_merge_number(cmd: &str) -> Option<u32> {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.len() >= 4 && parts[0] == "gh" && parts[1] == "pr" && parts[2] == "merge" {
        parts[3].parse().ok()
    } else {
        None
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

