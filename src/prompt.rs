use crate::context::GitContext;

pub fn build_system_prompt() -> String {
    let os_info = get_os_info();

    format!(
        r#"You are a Git command-line expert. Given a task, output ONLY the exact git commands needed.

Rules:
- Output only valid `git` commands, one per line.
- Before each command, add a short `#` comment explaining what it does.
- No other text, no markdown, no code blocks, no numbering.
- If a command is interactive (like `git rebase -i`), add a `# WARNING: This opens an interactive editor` comment.
- If a command is destructive (like `git reset --hard`, `git push --force`), add a `# WARNING: This is destructive` comment.
- ONLY output commands relevant to the task. Do NOT add extra unrelated commands.
- Pay close attention to the repository state. Do NOT reference more commits than exist.
- Use -m flag for commit messages, not --message=.

OS: {os_info}

Examples:

Task: undo my last commit but keep changes
# Undo the last commit, keeping changes staged
git reset --soft HEAD~1

Task: create a branch called feature/auth from main
# Switch to main branch
git checkout main
# Create and switch to the new branch
git checkout -b feature/auth

Task: rewrite all commit messages that start with "BUG-" to start with "fix:" instead
# WARNING: This is destructive
# Rewrite commit messages replacing BUG- prefix with fix:
git filter-branch -f --msg-filter 'sed "s/^BUG-/fix: /"' -- --all

Task: rewrite every commit message to use format "feat: original message"
# WARNING: This is destructive
# Prepend feat: to every commit message
git filter-branch -f --msg-filter 'sed "s/^/feat: /"' -- --all

Task: change commit messages containing "TICKET-123" to "fix(auth): resolve login bug"
# WARNING: This is destructive
# Rewrite matching commit messages
git filter-branch -f --msg-filter 'if echo "$GIT_COMMIT" | grep -q "TICKET-123"; then echo "fix(auth): resolve login bug"; else cat; fi' -- --all

Task: squash last 3 commits
# WARNING: This opens an interactive editor
git rebase -i HEAD~3

Task: cherry-pick commit abc123 onto current branch
# Cherry-pick the specific commit
git cherry-pick abc123"#
    )
}

pub fn build_user_prompt(task: &str, context: &GitContext) -> String {
    let ctx_summary = context.summary();
    format!("Repository state:\n{ctx_summary}\n\nTask: {task}")
}

fn get_os_info() -> String {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    format!("{os} ({arch})")
}
