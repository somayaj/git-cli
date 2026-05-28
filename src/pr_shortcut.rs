use crate::context::GitContext;

const PROTECTED_BRANCHES: &[&str] = &["main", "master", "develop"];

/// Simple "create a PR" tasks get deterministic git/gh commands (no LLM).
pub fn try_simple_pr_create(task: &str, ctx: &GitContext) -> Option<Result<String, String>> {
    if !ctx.is_repo {
        return None;
    }

    let lower = task.trim().to_lowercase();
    if !is_simple_create_intent(&lower) {
        return None;
    }

    let head = match ctx.branch.clone() {
        Some(b) => b,
        None => {
            return Some(Err(
                "Could not determine current branch. Run this inside a git repository."
                    .to_string(),
            ));
        }
    };

    if PROTECTED_BRANCHES.contains(&head.as_str()) {
        return Some(Err(format!(
            "You are on `{head}`. PRs need a feature branch.\n\n\
             Example:\n  \
             git checkout -b feature/my-change\n  \
             git push origin feature/my-change\n  \
             git-cli \"create a PR to main\" --execute"
        )));
    }

    let base = parse_target_base(&lower).unwrap_or_else(|| default_base_branch(ctx));
    let title = title_from_branch(&head);
    let body = format!("PR for branch {head}");

    let mut lines = vec![
        format!("# Push branch `{head}` to remote"),
        format!("git push origin {head}"),
        format!("# Create PR: {head} → {base}"),
        format!(
            "gh pr create --base {base} --head {head} --title \"{title}\" --body \"{body}\""
        ),
    ];

    if let Some(ref prs) = ctx.open_prs {
        let pattern = format!("{head} → {base}");
        if prs.contains(&pattern) {
            lines.insert(
                0,
                format!("# PR already exists for {head} → {base} (see Open PRs in context)"),
            );
            return Some(Ok(lines.join("\n")));
        }
    }

    Some(Ok(lines.join("\n")))
}

fn is_simple_create_intent(lower: &str) -> bool {
    const EXACT: &[&str] = &[
        "create a pr",
        "create pr",
        "create a pull request",
        "create pull request",
        "open a pr",
        "open pr",
    ];
    const PREFIX: &[&str] = &[
        "create a pr to ",
        "create pr to ",
        "create a pull request to ",
        "create pull request to ",
        "open a pr to ",
        "open pr to ",
    ];

    if EXACT.contains(&lower) {
        return true;
    }
    if PREFIX.iter().any(|p| lower.starts_with(p)) {
        // exclude multi-target / merge workflows
        return !lower.contains(" and ")
            && !lower.contains("merge")
            && !lower.contains("all ");
    }
    false
}

fn parse_target_base(lower: &str) -> Option<String> {
    const PREFIXES: &[&str] = &[
        "create a pr to ",
        "create pr to ",
        "create a pull request to ",
        "create pull request to ",
        "open a pr to ",
        "open pr to ",
    ];
    for prefix in PREFIXES {
        if let Some(rest) = lower.strip_prefix(prefix) {
            let base = rest.trim();
            if !base.is_empty() {
                return Some(base.to_string());
            }
        }
    }
    None
}

fn default_base_branch(ctx: &GitContext) -> String {
    if let Some(ref branches) = ctx.branches {
        if branches.lines().any(|l| l.trim().trim_start_matches('*').trim() == "main") {
            return "main".to_string();
        }
        if branches.lines().any(|l| l.trim().trim_start_matches('*').trim() == "master") {
            return "master".to_string();
        }
    }
    "main".to_string()
}

fn title_from_branch(head: &str) -> String {
    if let Some(name) = head.strip_prefix("feature/") {
        format!("feat: {name}")
    } else if let Some(name) = head.strip_prefix("fix/") {
        format!("fix: {name}")
    } else {
        format!("feat: {head}")
    }
}
