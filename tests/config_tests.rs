use git_cli::config::{is_complex_task, is_pr_task};

#[test]
fn pr_task_create_a_pr() {
    assert!(is_pr_task("create a pr"));
    assert!(is_complex_task("create a pr"));
}

#[test]
fn pr_task_variants() {
    assert!(is_pr_task("create pr to main"));
    assert!(is_pr_task("open a pr"));
    assert!(is_pr_task("merge pr #5"));
    assert!(is_pr_task("list open pull requests"));
}

#[test]
fn pr_task_does_not_match_unrelated() {
    assert!(!is_pr_task("show status"));
    assert!(!is_pr_task("create branch feature/auth"));
}
