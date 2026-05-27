use git_cli::executor::*;

// ── shell_split ──────────────────────────────────────────────────

#[test]
fn shell_split_simple() {
    assert_eq!(shell_split("git status"), vec!["git", "status"]);
}

#[test]
fn shell_split_double_quotes() {
    assert_eq!(
        shell_split(r#"git commit -m "fix: hello world""#),
        vec!["git", "commit", "-m", "fix: hello world"]
    );
}

#[test]
fn shell_split_single_quotes() {
    assert_eq!(
        shell_split("git log --format='%an' main"),
        vec!["git", "log", "--format=%an", "main"]
    );
}

#[test]
fn shell_split_mixed_quotes() {
    assert_eq!(
        shell_split(r#"git filter-branch --msg-filter 'sed "s/^BUG-/fix: /"' -- --all"#),
        vec!["git", "filter-branch", "--msg-filter", r#"sed "s/^BUG-/fix: /""#, "--", "--all"]
    );
}

#[test]
fn shell_split_empty() {
    let result: Vec<String> = Vec::new();
    assert_eq!(shell_split(""), result);
}

#[test]
fn shell_split_extra_spaces() {
    assert_eq!(
        shell_split("git   push   origin   main"),
        vec!["git", "push", "origin", "main"]
    );
}

// ── strip_inline_comment ─────────────────────────────────────────

#[test]
fn strip_inline_comment_basic() {
    assert_eq!(
        strip_inline_comment("gh pr merge 5 --merge # merge the PR"),
        "gh pr merge 5 --merge"
    );
}

#[test]
fn strip_inline_comment_preserves_pr_number() {
    assert_eq!(
        strip_inline_comment("gh pr merge #36 --merge"),
        "gh pr merge #36 --merge"
    );
}

#[test]
fn strip_inline_comment_no_comment() {
    assert_eq!(
        strip_inline_comment("git push origin main"),
        "git push origin main"
    );
}

#[test]
fn strip_inline_comment_hash_in_quotes() {
    assert_eq!(
        strip_inline_comment(r#"git commit -m "fix #123: bug""#),
        r#"git commit -m "fix #123: bug""#
    );
}

// ── find_unquoted_hash ───────────────────────────────────────────

#[test]
fn find_unquoted_hash_comment_after_space() {
    assert_eq!(find_unquoted_hash("git status # check"), Some(11));
}

#[test]
fn find_unquoted_hash_pr_number_not_comment() {
    assert_eq!(find_unquoted_hash("gh pr merge #36 --merge"), None);
}

#[test]
fn find_unquoted_hash_inside_quotes() {
    assert_eq!(find_unquoted_hash(r#"git commit -m "fix #1""#), None);
}

// ── has_placeholder ──────────────────────────────────────────────

#[test]
fn has_placeholder_detects_angle_brackets() {
    assert!(has_placeholder("gh pr merge <number> --merge"));
}

#[test]
fn has_placeholder_ignores_quoted() {
    assert!(!has_placeholder(r#"git commit -m "fix <issue>""#));
}

#[test]
fn has_placeholder_no_placeholder() {
    assert!(!has_placeholder("git push origin main"));
}

// ── strip_quoted_sections ────────────────────────────────────────

#[test]
fn strip_quoted_double() {
    assert_eq!(
        strip_quoted_sections(r#"git commit -m "hello world""#),
        "git commit -m "
    );
}

#[test]
fn strip_quoted_single() {
    assert_eq!(
        strip_quoted_sections("git log --format='%an' main"),
        "git log --format= main"
    );
}

// ── is_safe_command ──────────────────────────────────────────────

#[test]
fn safe_command_basic_git() {
    assert!(is_safe_command("git status"));
}

#[test]
fn safe_command_gh_allowed() {
    assert!(is_safe_command(
        "gh pr create --base main --head feature/x --title \"t\" --body \"b\""
    ));
}

#[test]
fn safe_command_blocks_injection_semicolon() {
    assert!(!is_safe_command("git status; rm -rf /"));
}

#[test]
fn safe_command_blocks_injection_ampersand() {
    assert!(!is_safe_command("git status && echo pwned"));
}

#[test]
fn safe_command_blocks_pipe() {
    assert!(!is_safe_command("git log | wc -l"));
}

#[test]
fn safe_command_allows_pipe_in_quotes() {
    assert!(is_safe_command(
        "git filter-branch --msg-filter 'sed \"s/^BUG-/fix: /\"' -- --all"
    ));
}

#[test]
fn safe_command_blocks_rebase_interactive() {
    assert!(!is_safe_command("git rebase -i HEAD~3"));
}

#[test]
fn safe_command_blocks_refspec_push() {
    assert!(!is_safe_command("git push origin feature:main"));
}

#[test]
fn safe_command_allows_delete_refspec() {
    assert!(is_safe_command("git push origin :feature/old"));
}

#[test]
fn safe_command_rejects_non_git() {
    assert!(!is_safe_command("rm -rf /"));
}

// ── extract_bad_flag ─────────────────────────────────────────────

#[test]
fn extract_bad_flag_unrecognized() {
    assert_eq!(
        extract_bad_flag("fatal: unrecognized argument: --vittles"),
        Some("--vittles".to_string())
    );
}

#[test]
fn extract_bad_flag_unknown_option() {
    assert_eq!(
        extract_bad_flag("error: unknown option 'count'"),
        Some("--count".to_string())
    );
}

#[test]
fn extract_bad_flag_branch_name_error() {
    assert_eq!(
        extract_bad_flag(
            "fatal: The -a, and -r, options to 'git branch' do not take a branch name."
        ),
        Some("__strip_trailing_arg__".to_string())
    );
}

#[test]
fn extract_bad_flag_no_error() {
    assert_eq!(extract_bad_flag("Everything up-to-date"), None);
}

// ── remove_flag ──────────────────────────────────────────────────

#[test]
fn remove_flag_simple() {
    assert_eq!(
        remove_flag("git log --vittles --oneline", "--vittles"),
        "git log --oneline"
    );
}

#[test]
fn remove_flag_strip_trailing_arg() {
    assert_eq!(
        remove_flag("git branch -r origin", "__strip_trailing_arg__"),
        "git branch -r"
    );
}

#[test]
fn remove_flag_preserves_quotes() {
    assert_eq!(
        remove_flag("git log --vittles --format='%an' main", "--vittles"),
        "git log --format='%an' main"
    );
}

// ── extract_pr_merge_number ──────────────────────────────────────

#[test]
fn extract_pr_merge_number_valid() {
    assert_eq!(extract_pr_merge_number("gh pr merge 42 --merge"), Some(42));
}

#[test]
fn extract_pr_merge_number_with_delete() {
    assert_eq!(
        extract_pr_merge_number("gh pr merge 10 --merge --delete-branch"),
        Some(10)
    );
}

#[test]
fn extract_pr_merge_number_missing() {
    assert_eq!(extract_pr_merge_number("gh pr merge --merge"), None);
}

#[test]
fn extract_pr_merge_number_not_a_number() {
    assert_eq!(extract_pr_merge_number("gh pr merge abc --merge"), None);
}

// ── parse_pr_number_from_output ──────────────────────────────────

#[test]
fn parse_pr_number_from_url() {
    assert_eq!(
        parse_pr_number_from_output("https://github.com/somayaj/repo-1/pull/40"),
        Some(40)
    );
}

#[test]
fn parse_pr_number_multiline() {
    let output = "Creating pull request...\nhttps://github.com/org/repo/pull/123\n";
    assert_eq!(parse_pr_number_from_output(output), Some(123));
}

#[test]
fn parse_pr_number_no_url() {
    assert_eq!(parse_pr_number_from_output("Everything up-to-date"), None);
}

// ── extract_head_branch ──────────────────────────────────────────

#[test]
fn extract_head_branch_found() {
    assert_eq!(
        extract_head_branch(
            "gh pr create --base main --head feature/auth --title \"t\" --body \"b\""
        ),
        Some("feature/auth".to_string())
    );
}

#[test]
fn extract_head_branch_missing() {
    assert_eq!(
        extract_head_branch("gh pr create --base main --title \"t\" --body \"b\""),
        None
    );
}

#[test]
fn fix_gh_pr_create_head_replaces_unknown_branch() {
    let cmd = "gh pr create --base main --head feature/my-change --title \"t\" --body \"b\"";
    let fixed = fix_gh_pr_create_head(cmd);
    assert!(!fixed.contains("feature/my-change"));
    assert!(fixed.contains("--head main"));
}

// ── strip_numbering ──────────────────────────────────────────────

#[test]
fn strip_numbering_dot() {
    assert_eq!(strip_numbering("1. git status"), Some("git status"));
}

#[test]
fn strip_numbering_paren() {
    assert_eq!(strip_numbering("2) git push"), Some("git push"));
}

#[test]
fn strip_numbering_colon() {
    assert_eq!(strip_numbering("3: git log"), Some("git log"));
}

#[test]
fn strip_numbering_none() {
    assert_eq!(strip_numbering("git status"), None);
}

// ── fix_case_globs ───────────────────────────────────────────────

#[test]
fn fix_case_globs_adds_star() {
    let input = "case \"$GIT_COMMIT\" in abc1234) echo \"fix\";; esac";
    let result = fix_case_globs(input);
    assert!(result.contains("abc1234*)"));
}

#[test]
fn fix_case_globs_already_has_star() {
    let input = "abc1234*) echo \"fix\"";
    let result = fix_case_globs(input);
    assert_eq!(result.matches('*').count(), 1);
}

// ── is_cherry_pick_in_pr_context ─────────────────────────────────

#[test]
fn cherry_pick_blocked_in_pr_context() {
    assert!(is_cherry_pick_in_pr_context(
        "git cherry-pick abc123",
        "gh pr create --base main\ngh pr merge 1 --merge"
    ));
}

#[test]
fn cherry_pick_allowed_without_pr() {
    assert!(!is_cherry_pick_in_pr_context(
        "git cherry-pick abc123",
        "git push origin main"
    ));
}

#[test]
fn non_cherry_pick_not_blocked() {
    assert!(!is_cherry_pick_in_pr_context(
        "git push origin main",
        "gh pr create --base main"
    ));
}

// ── is_checkout_for_cherry_pick ──────────────────────────────────

#[test]
fn checkout_blocked_in_cherry_pick_pr_context() {
    assert!(is_checkout_for_cherry_pick(
        "git checkout v15",
        "git cherry-pick abc123\ngh pr create --base v15"
    ));
}

#[test]
fn checkout_allowed_no_cherry_pick() {
    assert!(!is_checkout_for_cherry_pick(
        "git checkout main",
        "git push origin main"
    ));
}

// ── has_destructive_commands ──────────────────────────────────────

#[test]
fn detects_destructive_force_push() {
    let parsed = parse_response("git push --force origin main");
    assert!(has_destructive_commands(&parsed));
}

#[test]
fn detects_destructive_reset_hard() {
    let parsed = parse_response("git reset --hard HEAD~1");
    assert!(has_destructive_commands(&parsed));
}

#[test]
fn detects_destructive_branch_delete() {
    let parsed = parse_response("git branch -D feature/old");
    assert!(has_destructive_commands(&parsed));
}

#[test]
fn non_destructive_passes() {
    let parsed = parse_response("git status\ngit log --oneline");
    assert!(!has_destructive_commands(&parsed));
}

// ── parse_response ───────────────────────────────────────────────

#[test]
fn parse_response_separates_comments_and_commands() {
    let parsed =
        parse_response("# Switch to main\ngit checkout main\n# Push\ngit push origin main");
    let comments: Vec<_> = parsed
        .lines
        .iter()
        .filter(|l| matches!(l, OutputLine::Comment(_)))
        .collect();
    let commands: Vec<_> = parsed
        .lines
        .iter()
        .filter(|l| matches!(l, OutputLine::GitCommand(_)))
        .collect();
    assert_eq!(comments.len(), 2);
    assert_eq!(commands.len(), 2);
}

#[test]
fn parse_response_strips_code_fences() {
    let parsed = parse_response("```bash\ngit status\n```");
    let commands: Vec<_> = parsed
        .lines
        .iter()
        .filter(|l| matches!(l, OutputLine::GitCommand(_)))
        .collect();
    assert_eq!(commands.len(), 1);
}

#[test]
fn parse_response_strips_numbering() {
    let parsed = parse_response("1. git status\n2. git log");
    let commands: Vec<_> = parsed
        .lines
        .iter()
        .filter(|l| matches!(l, OutputLine::GitCommand(_)))
        .collect();
    assert_eq!(commands.len(), 2);
}

#[test]
fn parse_response_blocks_placeholder() {
    let parsed = parse_response("gh pr merge <number> --merge");
    let blocked: Vec<_> = parsed
        .lines
        .iter()
        .filter(|l| matches!(l, OutputLine::Other(_)))
        .collect();
    assert_eq!(blocked.len(), 1);
}

#[test]
fn parse_response_blocks_injection() {
    let parsed = parse_response("git status; rm -rf /");
    let blocked: Vec<_> = parsed
        .lines
        .iter()
        .filter(|l| matches!(l, OutputLine::Other(_)))
        .collect();
    assert_eq!(blocked.len(), 1);
}

// ── join_multiline_commands ──────────────────────────────────────

#[test]
fn join_multiline_unclosed_quote() {
    let lines = vec![
        "git filter-branch --msg-filter 'case \"$GIT_COMMIT\" in".to_string(),
        "abc*) echo \"fix\";; esac'".to_string(),
    ];
    let result = join_multiline_commands(&lines);
    assert_eq!(result.len(), 1);
    assert!(result[0].contains("esac'"));
}

#[test]
fn join_multiline_complete_lines() {
    let lines = vec!["git status".to_string(), "git log".to_string()];
    let result = join_multiline_commands(&lines);
    assert_eq!(result.len(), 2);
}

// ── find_unquoted_pipe ───────────────────────────────────────────

#[test]
fn find_pipe_outside_quotes() {
    assert_eq!(find_unquoted_pipe("git log | wc -l"), Some(8));
}

#[test]
fn find_pipe_inside_quotes() {
    assert_eq!(find_unquoted_pipe("git log --format='a|b'"), None);
}

#[test]
fn find_pipe_no_pipe() {
    assert_eq!(find_unquoted_pipe("git status"), None);
}

// ── extract_head_offset ──────────────────────────────────────────

#[test]
fn extract_head_offset_present() {
    assert_eq!(extract_head_offset("git reset --soft HEAD~3"), Some(3));
}

#[test]
fn extract_head_offset_absent() {
    assert_eq!(extract_head_offset("git status"), None);
}
