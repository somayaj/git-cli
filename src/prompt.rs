use crate::context::GitContext;

pub fn build_prompt(task: &str, context: &GitContext) -> String {
    let ctx_summary = context.summary();
    let os_info = get_os_info();

    format!(
        r#"You are a Git command-line expert. Given a task, output ONLY the git commands needed.

Rules:
- Output only valid `git` commands, one per line.
- Before each command, add a short `#` comment explaining what it does.
- No other text, no markdown, no code blocks, no numbering.
- If a command is interactive (like `git rebase -i`), add a `# WARNING: This opens an interactive editor` comment.
- If a command is destructive (like `git reset --hard`, `git push --force`), add a `# WARNING: This is destructive and cannot be undone` comment.
- List commands in correct execution order.

OS: {os_info}

Examples:

Task: undo my last commit but keep changes
# Undo the last commit, keeping changes staged
git reset --soft HEAD~1

Task: create a branch called feature/auth from main
# Switch to main branch
git checkout main
# Pull latest changes
git pull origin main
# Create and switch to the new branch
git checkout -b feature/auth

Task: squash last 3 commits
# WARNING: This opens an interactive editor
git rebase -i HEAD~3

Current repository state:
{ctx_summary}

Task: {task}"#
    )
}

fn get_os_info() -> String {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    format!("{os} ({arch})")
}
