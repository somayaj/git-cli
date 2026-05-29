use super::flags::{
    extract_bad_flag, extract_head_branch, extract_pr_merge_number, fix_gh_pr_create_head,
    parse_pr_number_from_output, remove_flag,
};
use super::parse::{GhAction, GitAction, PlannedAction, ResetMode};
use super::quote::shell_split;
use colored::Colorize;
use std::collections::HashMap;
use std::process::{Command, Output};

pub enum Executable {
    Typed(PlannedAction),
    Legacy(String),
}

impl Executable {
    fn uses_gh(&self) -> bool {
        match self {
            Executable::Typed(PlannedAction::Gh(_)) => true,
            Executable::Legacy(s) => s.starts_with("gh "),
            _ => false,
        }
    }

    fn is_gh_pr_create(&self) -> bool {
        match self {
            Executable::Typed(PlannedAction::Gh(GhAction::PrCreate { .. })) => true,
            Executable::Legacy(s) => s.starts_with("gh pr create"),
            _ => false,
        }
    }

    fn is_gh_pr_merge(&self) -> bool {
        match self {
            Executable::Typed(PlannedAction::Gh(GhAction::PrMerge { .. })) => true,
            Executable::Legacy(s) => s.starts_with("gh pr merge"),
            _ => false,
        }
    }
}

struct ExecCtx {
    pr_number_map: HashMap<u32, u32>,
    created_prs: Vec<u32>,
    predicted_merge_numbers: Vec<u32>,
    branch_pushed: bool,
    failed_cmds: Vec<String>,
}

pub fn execute_all(
    executables: Vec<Executable>,
    force: bool,
    has_destructive: bool,
) -> Result<(), String> {
    if executables.is_empty() {
        println!("{}", "No git commands found to execute.".yellow());
        return Ok(());
    }

    if executables.iter().any(|e| e.uses_gh()) && !crate::doctor::gh_on_path() {
        return Err(
            "GitHub CLI (gh) not found on PATH. Install: https://cli.github.com — then run `gh auth login`"
                .to_string(),
        );
    }

    if !force && has_destructive {
        eprintln!(
            "  {} Contains destructive commands. Use {} to override.",
            "Blocked:".red().bold(),
            "--force".bold()
        );
        return Ok(());
    }

    let has_creates = executables.iter().any(|e| e.is_gh_pr_create());
    let has_merges = executables.iter().any(|e| e.is_gh_pr_merge());

    let predicted_merge_numbers: Vec<u32> = if has_creates && has_merges {
        let open_prs = get_open_pr_numbers();
        executables
            .iter()
            .filter_map(|e| match e {
                Executable::Typed(PlannedAction::Gh(GhAction::PrMerge { number, .. })) => {
                    Some(*number)
                }
                Executable::Legacy(s) => extract_pr_merge_number(s),
                _ => None,
            })
            .filter(|n| !open_prs.contains(n))
            .collect()
    } else {
        Vec::new()
    };

    let mut ctx = ExecCtx {
        pr_number_map: HashMap::new(),
        created_prs: Vec::new(),
        predicted_merge_numbers,
        branch_pushed: false,
        failed_cmds: Vec::new(),
    };

    for exec in executables {
        match exec {
            Executable::Typed(action) => run_typed(action, &mut ctx)?,
            Executable::Legacy(cmd) => run_legacy(&cmd, &mut ctx)?,
        }
    }

    if has_creates || has_merges {
        auto_merge_remaining_prs();
    }

    finish_exec(&ctx)
}

fn finish_exec(ctx: &ExecCtx) -> Result<(), String> {
    if ctx.failed_cmds.is_empty() {
        println!("  {}", "All commands completed successfully.".green().bold());
        Ok(())
    } else {
        eprintln!();
        eprintln!(
            "  {} {} command(s) failed:",
            "Summary:".yellow().bold(),
            ctx.failed_cmds.len()
        );
        for cmd in &ctx.failed_cmds {
            eprintln!("    {} {}", "✗".red(), cmd);
        }
        eprintln!();
        Err(format!("{} command(s) failed (see above)", ctx.failed_cmds.len()))
    }
}

fn run_typed(action: PlannedAction, ctx: &mut ExecCtx) -> Result<(), String> {
    let action = prepare_typed(action, ctx);

    if let PlannedAction::Gh(GhAction::PrCreate { head, .. }) = &action {
        if !ctx.branch_pushed && !head.is_empty() {
            auto_push_branch(head);
            ctx.branch_pushed = true;
        }
    }

    let display = action_display(&action);
    println!("  {} {}", "Running:".cyan().bold(), display);

    let (output, final_display) = run_argv_with_flag_retry(&action)?;

    print_output(&output);

    if output.status.success() {
        if let PlannedAction::Gh(GhAction::PrCreate { .. }) = &action {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(pr_num) = parse_pr_number_from_output(&stdout) {
                let idx = ctx.created_prs.len();
                ctx.created_prs.push(pr_num);
                if let Some(&predicted) = ctx.predicted_merge_numbers.get(idx) {
                    ctx.pr_number_map.insert(predicted, pr_num);
                }
            }
        }
        return Ok(());
    }

    handle_typed_failure(&action, &final_display, &output, ctx)
}

fn prepare_typed(action: PlannedAction, ctx: &ExecCtx) -> PlannedAction {
    match action {
        PlannedAction::Gh(GhAction::PrCreate {
            base,
            mut head,
            title,
            body,
        }) => {
            if head.is_empty() || !branch_exists(&head) {
                if let Some(current) = current_branch() {
                    if head != current {
                        eprintln!(
                            "  {} Replacing hallucinated --head `{}` with current branch `{}`",
                            "Auto:".cyan().bold(),
                            if head.is_empty() { "(missing)" } else { &head },
                            current
                        );
                    }
                    head = current;
                }
            }
            PlannedAction::Gh(GhAction::PrCreate {
                base,
                head,
                title,
                body,
            })
        }
        PlannedAction::Gh(GhAction::PrMerge {
            number,
            merge,
            delete_branch,
        }) => {
            if let Some(&actual) = ctx.pr_number_map.get(&number) {
                eprintln!(
                    "  {} PR #{} → #{} (actual)",
                    "Remapped:".yellow().bold(),
                    number,
                    actual
                );
                PlannedAction::Gh(GhAction::PrMerge {
                    number: actual,
                    merge,
                    delete_branch,
                })
            } else {
                PlannedAction::Gh(GhAction::PrMerge {
                    number,
                    merge,
                    delete_branch,
                })
            }
        }
        other => other,
    }
}

fn run_legacy(cmd: &str, ctx: &mut ExecCtx) -> Result<(), String> {
    let mut actual_cmd = if cmd.starts_with("gh pr merge") {
        if let Some(n) = extract_pr_merge_number(cmd) {
            if let Some(&actual) = ctx.pr_number_map.get(&n) {
                let replaced = cmd.replacen(&n.to_string(), &actual.to_string(), 1);
                eprintln!(
                    "  {} PR #{} → #{} (actual)",
                    "Remapped:".yellow().bold(),
                    n,
                    actual
                );
                replaced
            } else {
                cmd.to_string()
            }
        } else if !ctx.created_prs.is_empty() {
            let last_pr = ctx.created_prs[ctx.created_prs.len() - 1];
            let fixed = cmd.replacen("gh pr merge", &format!("gh pr merge {last_pr}"), 1);
            eprintln!(
                "  {} Injecting PR #{} (last created)",
                "Auto:".cyan().bold(),
                last_pr
            );
            fixed
        } else {
            cmd.to_string()
        }
    } else {
        cmd.to_string()
    };

    if actual_cmd.starts_with("gh pr create") {
        actual_cmd = fix_gh_pr_create_head(&actual_cmd);
    }

    if actual_cmd.starts_with("gh pr create") && !ctx.branch_pushed {
        if let Some(branch) = extract_head_branch(&actual_cmd) {
            auto_push_branch(&branch);
            ctx.branch_pushed = true;
        }
    }

    println!("  {} {}", "Running:".cyan().bold(), actual_cmd);

    let parts = shell_split(&actual_cmd);
    if parts.is_empty() {
        return Ok(());
    }

    let (output, actual_cmd) = run_legacy_with_flag_retry(&actual_cmd)?;
    print_output(&output);

    if output.status.success() {
        if cmd.starts_with("gh pr create") {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(pr_num) = parse_pr_number_from_output(&stdout) {
                let idx = ctx.created_prs.len();
                ctx.created_prs.push(pr_num);
                if let Some(&predicted) = ctx.predicted_merge_numbers.get(idx) {
                    ctx.pr_number_map.insert(predicted, pr_num);
                }
            }
        }
        return Ok(());
    }

    handle_legacy_failure(&actual_cmd, &parts, &output, ctx)
}

pub fn action_display(action: &PlannedAction) -> String {
    let (prog, args) = action_argv(action);
    format_argv_line(&prog, &args)
}

pub fn action_argv(action: &PlannedAction) -> (String, Vec<String>) {
    match action {
        PlannedAction::Git(git) => git_argv(git),
        PlannedAction::Gh(gh) => gh_argv(gh),
    }
}

fn git_argv(git: &GitAction) -> (String, Vec<String>) {
    let mut args = vec!["git".to_string()];
    match git {
        GitAction::Status => args.push("status".to_string()),
        GitAction::Add { paths } => {
            args.push("add".to_string());
            args.extend(paths.clone());
        }
        GitAction::Commit { message, amend } => {
            args.push("commit".to_string());
            if *amend {
                args.push("--amend".to_string());
            }
            args.push("-m".to_string());
            args.push(message.clone());
        }
        GitAction::Checkout { branch, create } => {
            args.push("checkout".to_string());
            if *create {
                args.push("-b".to_string());
            }
            args.push(branch.clone());
        }
        GitAction::Push {
            remote,
            branch,
            force,
        } => {
            args.push("push".to_string());
            if *force {
                args.push("--force".to_string());
            }
            args.push(remote.clone());
            args.push(branch.clone());
        }
        GitAction::PushDeleteRemote { remote, branch } => {
            args.push("push".to_string());
            args.push(remote.clone());
            args.push(format!(":{branch}"));
        }
        GitAction::Log => {
            args.push("log".to_string());
        }
        GitAction::Reset { mode, target } => {
            args.push("reset".to_string());
            match mode {
                ResetMode::Soft => args.push("--soft".to_string()),
                ResetMode::Mixed => {}
                ResetMode::Hard => args.push("--hard".to_string()),
            }
            args.push(target.clone());
        }
        GitAction::BranchDelete { name } => {
            args.push("branch".to_string());
            args.push("-D".to_string());
            args.push(name.clone());
        }
        GitAction::Clean { extra, .. } => {
            args.push("clean".to_string());
            args.extend(extra.clone());
        }
    }
    let prog = args.remove(0);
    (prog, args)
}

fn gh_argv(gh: &GhAction) -> (String, Vec<String>) {
    let mut args = vec!["gh".to_string()];
    match gh {
        GhAction::PrCreate {
            base,
            head,
            title,
            body,
        } => {
            args.extend([
                "pr".to_string(),
                "create".to_string(),
                "--base".to_string(),
                base.clone(),
                "--title".to_string(),
                title.clone(),
                "--body".to_string(),
                body.clone(),
            ]);
            if !head.is_empty() {
                args.push("--head".to_string());
                args.push(head.clone());
            }
        }
        GhAction::PrMerge {
            number,
            merge,
            delete_branch,
        } => {
            args.extend([
                "pr".to_string(),
                "merge".to_string(),
                number.to_string(),
            ]);
            if *merge {
                args.push("--merge".to_string());
            }
            if *delete_branch {
                args.push("--delete-branch".to_string());
            }
        }
    }
    let prog = args.remove(0);
    (prog, args)
}

fn format_argv_line(prog: &str, args: &[String]) -> String {
    let mut parts = vec![prog.to_string()];
    for a in args {
        parts.push(if a.contains(' ') {
            format!("\"{a}\"")
        } else {
            a.clone()
        });
    }
    parts.join(" ")
}

fn run_argv_with_flag_retry(action: &PlannedAction) -> Result<(Output, String), String> {
    let (prog, mut args) = action_argv(action);
    let mut display = action_display(action);

    for _ in 0..3 {
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let output = Command::new(&prog)
            .args(&arg_refs)
            .output()
            .map_err(|e| format!("Failed to run `{display}`: {e}"))?;

        if output.status.success() {
            return Ok((output, display));
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        if let Some(bad_flag) = extract_bad_flag(&stderr) {
            eprintln!(
                "  {} Removing hallucinated flag `{}`",
                "Fix:".yellow().bold(),
                bad_flag
            );
            let cmd_line = format_argv_line(&prog, &args);
            let fixed = remove_flag(&cmd_line, &bad_flag);
            args = shell_split(&fixed);
            if !args.is_empty() {
                args.remove(0); // drop program name from shell_split
            }
            display = fixed;
            println!("  {} {}", "Retrying:".cyan().bold(), display);
        } else {
            return Ok((output, display));
        }
    }

    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = Command::new(&prog)
        .args(&arg_refs)
        .output()
        .map_err(|e| format!("Failed to run `{display}`: {e}"))?;
    Ok((output, display))
}

fn run_legacy_with_flag_retry(cmd: &str) -> Result<(Output, String), String> {
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

fn print_output(output: &Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stdout.trim().is_empty() {
        println!("{stdout}");
    }
    if !stderr.trim().is_empty() {
        eprintln!("{stderr}");
    }
}

fn handle_typed_failure(
    action: &PlannedAction,
    display: &str,
    output: &Output,
    ctx: &mut ExecCtx,
) -> Result<(), String> {
    let stderr = String::from_utf8_lossy(&output.stderr);

    if let PlannedAction::Git(GitAction::Checkout { branch, create: true }) = action {
        if stderr.contains("already exists") || stderr.contains("already exist") {
            eprintln!(
                "  {} Branch already exists, switching to it instead...",
                "Auto:".cyan().bold()
            );
            let retry = Command::new("git").args(["checkout", branch]).output();
            if let Ok(o) = retry {
                print_output(&o);
                if o.status.success() {
                    return Ok(());
                }
            }
        }
    }

    if let PlannedAction::Gh(GhAction::PrMerge { number, delete_branch, .. }) = action {
        let stderr_str = stderr.to_string();
        if stderr_str.contains("not allowed") || stderr_str.contains("not mergeable") {
            if retry_merge_typed(*number, *delete_branch).is_some() {
                return Ok(());
            }
        }
        eprintln!(
            "  {} `{}` failed (exit code {}). Continuing with remaining commands...",
            "Skipped:".yellow().bold(),
            display,
            output.status.code().unwrap_or(-1)
        );
        ctx.failed_cmds.push(display.to_string());
        return Ok(());
    }

    if let PlannedAction::Gh(GhAction::PrCreate { .. }) = action {
        eprintln!(
            "  {} `{}` failed (exit code {}). Continuing with remaining commands...",
            "Skipped:".yellow().bold(),
            display,
            output.status.code().unwrap_or(-1)
        );
        ctx.failed_cmds.push(display.to_string());
        return Ok(());
    }

    if let PlannedAction::Git(GitAction::Push { .. }) = action {
        if stderr.contains("non-fast-forward") || stderr.contains("already exists") {
            eprintln!(
                "  {} Push failed but branch likely exists on remote. Continuing...",
                "Note:".yellow().bold(),
            );
            return Ok(());
        }
    }

    if let PlannedAction::Git(GitAction::BranchDelete { .. }) = action {
        return handle_branch_delete_failure(display, &stderr, ctx);
    }

    if let PlannedAction::Git(GitAction::PushDeleteRemote { .. }) = action {
        eprintln!(
            "  {} Branch may already be deleted. Continuing...",
            "Note:".yellow().bold(),
        );
        return Ok(());
    }

    Err(format!(
        "Command `{display}` failed with exit code {}",
        output.status.code().unwrap_or(-1)
    ))
}

fn handle_legacy_failure(
    actual_cmd: &str,
    _parts: &[String],
    output: &Output,
    ctx: &mut ExecCtx,
) -> Result<(), String> {
    let stderr = String::from_utf8_lossy(&output.stderr);

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
            print_output(&o);
            if o.status.success() {
                return Ok(());
            }
        }
    }

    if actual_cmd.starts_with("gh pr merge") {
        let stderr_str = stderr.to_string();
        if stderr_str.contains("not allowed") || stderr_str.contains("not mergeable") {
            if retry_merge_legacy(actual_cmd).is_some() {
                return Ok(());
            }
        }
        eprintln!(
            "  {} `{}` failed (exit code {}). Continuing with remaining commands...",
            "Skipped:".yellow().bold(),
            actual_cmd,
            output.status.code().unwrap_or(-1)
        );
        ctx.failed_cmds.push(actual_cmd.to_string());
        return Ok(());
    }

    if actual_cmd.starts_with("gh pr create") {
        eprintln!(
            "  {} `{}` failed (exit code {}). Continuing with remaining commands...",
            "Skipped:".yellow().bold(),
            actual_cmd,
            output.status.code().unwrap_or(-1)
        );
        ctx.failed_cmds.push(actual_cmd.to_string());
        return Ok(());
    }

    if actual_cmd.starts_with("git push")
        && (stderr.contains("non-fast-forward") || stderr.contains("already exists"))
    {
        eprintln!(
            "  {} Push failed but branch likely exists on remote. Continuing...",
            "Note:".yellow().bold(),
        );
        return Ok(());
    }

    if actual_cmd.contains("branch -D") || actual_cmd.contains("branch -d") {
        return handle_branch_delete_failure(actual_cmd, &stderr, ctx);
    }

    if actual_cmd.contains("push origin --delete") || actual_cmd.contains("push origin :") {
        eprintln!(
            "  {} Branch may already be deleted. Continuing...",
            "Note:".yellow().bold(),
        );
        return Ok(());
    }

    Err(format!(
        "Command `{actual_cmd}` failed with exit code {}",
        output.status.code().unwrap_or(-1)
    ))
}

fn handle_branch_delete_failure(display: &str, stderr: &str, _ctx: &mut ExecCtx) -> Result<(), String> {
    if stderr.contains("checked out") || stderr.contains("Cannot delete") {
        eprintln!("  {} Switching to main before deleting...", "Auto:".cyan().bold());
        let _ = Command::new("git").args(["checkout", "main"]).output();
        let parts = shell_split(display);
        if parts.len() > 1 {
            let retry = Command::new(&parts[0]).args(&parts[1..]).output();
            if let Ok(o) = retry {
                print_output(&o);
                if o.status.success() {
                    return Ok(());
                }
            }
        }
    }
    eprintln!(
        "  {} Branch may already be deleted. Continuing...",
        "Note:".yellow().bold(),
    );
    Ok(())
}

fn retry_merge_typed(number: u32, delete_branch: bool) -> Option<()> {
    let number_str = number.to_string();
    for strategy in ["--squash", "--rebase"] {
        eprintln!(
            "  {} Retrying PR #{} with `{}`...",
            "Fallback:".cyan().bold(),
            number,
            strategy
        );
        let mut args = vec!["pr", "merge", number_str.as_str(), strategy];
        if delete_branch {
            args.push("--delete-branch");
        }
        let output = Command::new("gh").args(&args).output().ok()?;
        print_output(&output);
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

fn retry_merge_legacy(original_cmd: &str) -> Option<()> {
    for strategy in ["--squash", "--rebase"] {
        let retry_cmd = original_cmd.replace("--merge", strategy);
        eprintln!(
            "  {} Retrying with `{}`...",
            "Fallback:".cyan().bold(),
            strategy
        );
        let parts = shell_split(&retry_cmd);
        if parts.is_empty() {
            continue;
        }
        let output = Command::new(&parts[0]).args(&parts[1..]).output().ok()?;
        print_output(&output);
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

fn auto_push_branch(branch: &str) {
    eprintln!(
        "  {} Pushing branch `{}` to remote first...",
        "Auto:".cyan().bold(),
        branch
    );
    if let Ok(o) = Command::new("git").args(["push", "origin", branch]).output() {
        print_output(&o);
    }
}

fn branch_exists(name: &str) -> bool {
    Command::new("git")
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
    Command::new("git")
        .args(["branch", "--show-current"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

fn get_open_pr_numbers() -> Vec<u32> {
    Command::new("gh")
        .args([
            "pr",
            "list",
            "--state",
            "open",
            "--json",
            "number",
            "--template",
            "{{range .}}{{.number}}\n{{end}}",
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

fn auto_merge_remaining_prs() {
    let current_branch = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

    let Some(branch) = current_branch else {
        return;
    };

    let pr_output = Command::new("gh")
        .args([
            "pr",
            "list",
            "--state",
            "open",
            "--head",
            &branch,
            "--json",
            "number",
            "--template",
            "{{range .}}{{.number}}\n{{end}}",
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string());

    let Some(prs) = pr_output else {
        return;
    };
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
        eprintln!(
            "  {} gh pr merge {} --squash{}",
            "Running:".cyan().bold(),
            pr,
            if is_last { " --delete-branch" } else { "" }
        );

        let pr_str = pr.to_string();
        let mut args = vec!["pr", "merge", pr_str.as_str(), "--squash"];
        if is_last {
            args.push("--delete-branch");
        }
        let output = Command::new("gh").args(&args).output();
        if let Ok(o) = output {
            print_output(&o);
            if !o.status.success() {
                eprintln!(
                    "  {} PR #{} merge failed, trying --rebase...",
                    "Fallback:".yellow().bold(),
                    pr
                );
                let pr_str = pr.to_string();
                let mut retry_args = vec!["pr", "merge", &pr_str, "--rebase"];
                if is_last {
                    retry_args.push("--delete-branch");
                }
                if let Ok(o) = Command::new("gh").args(&retry_args).output() {
                    print_output(&o);
                }
            }
        }
    }
}
