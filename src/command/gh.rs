use super::parse::{BlockReason, GhAction, PlannedAction};

pub fn parse_gh(parts: &[String]) -> Result<PlannedAction, BlockReason> {
    if parts.len() < 3 || parts[1] != "pr" {
        return Err(BlockReason::InvalidGhSubcommand);
    }

    match parts[2].as_str() {
        "create" => parse_gh_pr_create(&parts[3..]),
        "merge" => parse_gh_pr_merge(&parts[3..]),
        _ => Err(BlockReason::InvalidGhSubcommand),
    }
}

fn parse_gh_pr_create(args: &[String]) -> Result<PlannedAction, BlockReason> {
    let base = flag_value(args, "--base").ok_or(BlockReason::MissingRequiredFlag)?;
    let head = flag_value(args, "--head");
    let title = flag_value(args, "--title").ok_or(BlockReason::MissingRequiredFlag)?;
    let body = flag_value(args, "--body").ok_or(BlockReason::MissingRequiredFlag)?;

    Ok(PlannedAction::Gh(GhAction::PrCreate {
        base,
        head: head.unwrap_or_default(),
        title,
        body,
    }))
}

fn parse_gh_pr_merge(args: &[String]) -> Result<PlannedAction, BlockReason> {
    let number = args
        .first()
        .and_then(|s| s.parse().ok())
        .ok_or(BlockReason::MissingRequiredFlag)?;

    let merge = args.iter().any(|a| a == "--merge" || a == "--squash" || a == "--rebase");
    let delete_branch = args.iter().any(|a| a == "--delete-branch");

    Ok(PlannedAction::Gh(GhAction::PrMerge {
        number,
        merge,
        delete_branch,
    }))
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}
