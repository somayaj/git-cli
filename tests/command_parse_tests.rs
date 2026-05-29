use git_cli::command::{
    action_argv, action_display, try_parse_command, BlockReason, GhAction, GitAction, PlannedAction,
};

#[test]
fn parse_git_status() {
    let action = try_parse_command("git status").unwrap();
    assert_eq!(action, PlannedAction::Git(GitAction::Status));
}

#[test]
fn parse_git_push_origin_branch() {
    let action = try_parse_command("git push origin feature/foo").unwrap();
    assert_eq!(
        action,
        PlannedAction::Git(GitAction::Push {
            remote: "origin".into(),
            branch: "feature/foo".into(),
            force: false,
        })
    );
}

#[test]
fn parse_git_push_force() {
    let action = try_parse_command("git push --force origin main").unwrap();
    assert!(matches!(
        action,
        PlannedAction::Git(GitAction::Push {
            force: true,
            ..
        })
    ));
}

#[test]
fn parse_git_checkout_create() {
    let action = try_parse_command("git checkout -b feature/x").unwrap();
    assert_eq!(
        action,
        PlannedAction::Git(GitAction::Checkout {
            branch: "feature/x".into(),
            create: true,
        })
    );
}

#[test]
fn parse_git_commit_with_message() {
    let action = try_parse_command(r#"git commit -m "fix: hello""#).unwrap();
    assert_eq!(
        action,
        PlannedAction::Git(GitAction::Commit {
            message: "fix: hello".into(),
            amend: false,
        })
    );
}

#[test]
fn parse_gh_pr_create() {
    let action = try_parse_command(
        r#"gh pr create --base main --head feature/x --title "t" --body "b""#,
    )
    .unwrap();
    assert_eq!(
        action,
        PlannedAction::Gh(GhAction::PrCreate {
            base: "main".into(),
            head: "feature/x".into(),
            title: "t".into(),
            body: "b".into(),
        })
    );
}

#[test]
fn parse_gh_pr_merge() {
    let action = try_parse_command("gh pr merge 42 --merge").unwrap();
    assert_eq!(
        action,
        PlannedAction::Gh(GhAction::PrMerge {
            number: 42,
            merge: true,
            delete_branch: false,
        })
    );
}

#[test]
fn blocks_injection() {
    assert_eq!(
        try_parse_command("git status; rm -rf /"),
        Err(BlockReason::InjectionPattern)
    );
}

#[test]
fn blocks_invalid_refspec_push() {
    assert_eq!(
        try_parse_command("git push origin feature:main"),
        Err(BlockReason::InvalidRefspec)
    );
}

#[test]
fn blocks_unsupported_gh() {
    assert_eq!(
        try_parse_command("gh auth login"),
        Err(BlockReason::InvalidGhSubcommand)
    );
}

#[test]
fn unknown_git_subcommand_for_fallback() {
    assert_eq!(
        try_parse_command("git filter-branch --all"),
        Err(BlockReason::UnknownSubcommand)
    );
}

#[test]
fn action_argv_git_push() {
    let action = try_parse_command("git push origin feature/foo").unwrap();
    let (prog, args) = action_argv(&action);
    assert_eq!(prog, "git");
    assert_eq!(args, ["push", "origin", "feature/foo"]);
}

#[test]
fn action_display_matches_command() {
    let action = try_parse_command("git status").unwrap();
    assert_eq!(action_display(&action), "git status");
}
