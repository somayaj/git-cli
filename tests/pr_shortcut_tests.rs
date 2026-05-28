use git_cli::context::GitContext;
use git_cli::pr_shortcut::try_simple_pr_create;

fn ctx_on_branch(branch: &str) -> GitContext {
    GitContext {
        is_repo: true,
        branch: Some(branch.to_string()),
        status: None,
        recent_log: None,
        remotes: None,
        branches: Some(format!("  main\n* {branch}\n")),
        open_prs: None,
        gh_warning: None,
    }
}

#[test]
fn simple_create_pr_generates_gh_commands() {
    let ctx = ctx_on_branch("feature/login");
    let result = try_simple_pr_create("create a pr", &ctx).unwrap().unwrap();
    assert!(result.contains("git push origin feature/login"));
    assert!(result.contains("gh pr create --base main --head feature/login"));
}

#[test]
fn create_pr_to_custom_base() {
    let ctx = ctx_on_branch("feature/login");
    let result = try_simple_pr_create("create a pr to develop", &ctx)
        .unwrap()
        .unwrap();
    assert!(result.contains("--base develop"));
}

#[test]
fn rejects_main_as_head() {
    let ctx = ctx_on_branch("main");
    let result = try_simple_pr_create("create a pr", &ctx).unwrap();
    assert!(result.is_err());
}

#[test]
fn complex_pr_task_uses_llm() {
    let ctx = ctx_on_branch("feature/login");
    assert!(try_simple_pr_create("create prs to main and v15 and merge", &ctx).is_none());
}
