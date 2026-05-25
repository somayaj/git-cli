use regex::Regex;

pub struct ShortcutResult {
    pub commands: Vec<(String, String)>, // (comment, command)
}

impl ShortcutResult {
    fn single(comment: &str, cmd: &str) -> Self {
        Self {
            commands: vec![(comment.to_string(), cmd.to_string())],
        }
    }

    fn multi(pairs: &[(&str, &str)]) -> Self {
        Self {
            commands: pairs
                .iter()
                .map(|(c, cmd)| (c.to_string(), cmd.to_string()))
                .collect(),
        }
    }

    pub fn to_response_string(&self) -> String {
        self.commands
            .iter()
            .map(|(comment, cmd)| format!("# {comment}\n{cmd}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub fn try_shortcut(input: &str) -> Option<ShortcutResult> {
    let lower = input.to_lowercase();
    // --- Status / Info ---
    if matches_any(&lower, &["show status", "git status", "check status", "what changed"]) {
        return Some(ShortcutResult::single("Show working tree status", "git status"));
    }

    if matches_any(&lower, &["show log", "recent commits", "commit history", "show history"]) {
        return Some(ShortcutResult::single(
            "Show recent commit history",
            "git log --oneline -20",
        ));
    }

    if matches_any(&lower, &["show diff", "what changed", "see changes", "show changes"]) && !lower.contains("staged") {
        return Some(ShortcutResult::single(
            "Show unstaged changes",
            "git diff",
        ));
    }

    if matches_any(&lower, &["staged diff", "staged changes", "show staged"]) {
        return Some(ShortcutResult::single(
            "Show staged changes",
            "git diff --cached",
        ));
    }

    if matches_any(&lower, &["show branches", "list branches", "all branches"]) {
        return Some(ShortcutResult::single(
            "List all branches",
            "git branch -a",
        ));
    }

    if matches_any(&lower, &["show remotes", "list remotes"]) {
        return Some(ShortcutResult::single(
            "List remotes with URLs",
            "git remote -v",
        ));
    }

    if matches_any(&lower, &["show tags", "list tags"]) {
        return Some(ShortcutResult::single("List all tags", "git tag -l"));
    }

    if matches_any(&lower, &["current branch", "which branch", "what branch"]) {
        return Some(ShortcutResult::single(
            "Show current branch name",
            "git rev-parse --abbrev-ref HEAD",
        ));
    }

    // --- Stash ---
    if lower == "stash" || matches_any(&lower, &["stash changes", "save stash", "stash everything"]) {
        return Some(ShortcutResult::single("Stash working changes", "git stash"));
    }

    if matches_any(&lower, &["pop stash", "apply stash", "unstash", "restore stash"]) {
        return Some(ShortcutResult::single(
            "Apply and remove the latest stash",
            "git stash pop",
        ));
    }

    if matches_any(&lower, &["list stash", "show stash", "stash list"]) {
        return Some(ShortcutResult::single("List all stashes", "git stash list"));
    }

    // --- Undo / Reset ---
    if (matches_any(&lower, &["undo last commit", "undo commit", "uncommit", "undo my last commit"]) 
        || extract_pattern(&lower, &[r"undo\s+(?:my\s+)?(?:the\s+)?last\s+commit"]).is_some())
        && !lower.contains("hard") {
        return Some(ShortcutResult::single(
            "Undo the last commit, keeping changes staged",
            "git reset --soft HEAD~1",
        ));
    }

    if matches_any(&lower, &["discard all changes", "reset everything", "throw away changes", "nuke changes"]) {
        return Some(ShortcutResult::multi(&[
            ("Unstage all files", "git reset HEAD"),
            ("Discard all working tree changes", "git checkout -- ."),
        ]));
    }

    if matches_any(&lower, &["unstage all", "unstage everything", "reset staging"]) {
        return Some(ShortcutResult::single("Unstage all files", "git reset HEAD"));
    }

    // --- Stage / Commit ---
    if matches_any(&lower, &["stage all", "add all", "add everything", "stage everything"]) {
        return Some(ShortcutResult::single("Stage all changes", "git add -A"));
    }

    if matches_any(&lower, &["amend commit", "amend last commit", "fix last commit message"]) {
        return Some(ShortcutResult::single(
            "Amend the last commit (opens editor)",
            "git commit --amend",
        ));
    }

    // --- Branch ---
    if let Some(branch_name) = extract_pattern(&lower, &[
        r"(?:create|make|new)\s+branch\s+(\S+)",
        r"checkout\s+(?:new|a new)\s+branch\s+(\S+)",
        r"branch\s+called\s+(\S+)",
    ]) {
        return Some(ShortcutResult::single(
            &format!("Create and switch to branch '{branch_name}'"),
            &format!("git checkout -b {branch_name}"),
        ));
    }

    if let Some(branch_name) = extract_pattern(&lower, &[
        r"(?:switch|checkout)\s+(?:to\s+)?(?:branch\s+)?(\S+)",
    ]) {
        if !["a", "new", "the", "my", "branch"].contains(&branch_name.as_str()) {
            return Some(ShortcutResult::single(
                &format!("Switch to branch '{branch_name}'"),
                &format!("git checkout {branch_name}"),
            ));
        }
    }

    if let Some(branch_name) = extract_pattern(&lower, &[
        r"delete\s+branch\s+(\S+)",
        r"remove\s+branch\s+(\S+)",
    ]) {
        return Some(ShortcutResult::single(
            &format!("Delete local branch '{branch_name}'"),
            &format!("git branch -d {branch_name}"),
        ));
    }

    // --- Pull / Push / Fetch ---
    if lower == "pull" || matches_any(&lower, &["pull latest", "pull changes", "git pull"]) {
        return Some(ShortcutResult::single("Pull latest changes", "git pull"));
    }

    if lower == "push" || matches_any(&lower, &["push changes", "git push", "push to remote"]) {
        return Some(ShortcutResult::single("Push to remote", "git push"));
    }

    if matches_any(&lower, &["push and set upstream", "push upstream", "push set tracking"]) {
        return Some(ShortcutResult::single(
            "Push and set upstream tracking branch",
            "git push -u origin HEAD",
        ));
    }

    if lower == "fetch" || matches_any(&lower, &["fetch all", "fetch changes", "git fetch"]) {
        return Some(ShortcutResult::single(
            "Fetch from all remotes",
            "git fetch --all",
        ));
    }

    // --- Merge / Rebase ---
    if let Some(branch) = extract_pattern(&lower, &[
        r"merge\s+(\S+)",
    ]) {
        if branch != "branch" && branch != "changes" {
            return Some(ShortcutResult::single(
                &format!("Merge branch '{branch}' into current branch"),
                &format!("git merge {branch}"),
            ));
        }
    }

    if matches_any(&lower, &["abort merge", "cancel merge"]) {
        return Some(ShortcutResult::single("Abort the current merge", "git merge --abort"));
    }

    if matches_any(&lower, &["abort rebase", "cancel rebase"]) {
        return Some(ShortcutResult::single("Abort the current rebase", "git rebase --abort"));
    }

    // --- Cherry-pick ---
    if matches_any(&lower, &["abort cherry", "cancel cherry"]) {
        return Some(ShortcutResult::single(
            "Abort the current cherry-pick",
            "git cherry-pick --abort",
        ));
    }

    // --- Clean ---
    if matches_any(&lower, &["clean untracked", "remove untracked", "delete untracked"]) {
        return Some(ShortcutResult::single(
            "WARNING: Remove all untracked files (dry run first)",
            "git clean -n",
        ));
    }

    // --- Squash (with number) ---
    if let Some(n) = extract_pattern(&lower, &[
        r"squash\s+(?:last\s+)?(\d+)\s+commits?",
    ]) {
        return Some(ShortcutResult::single(
            &format!("WARNING: Interactive rebase to squash last {n} commits"),
            &format!("git rebase -i HEAD~{n}"),
        ));
    }

    // --- Blame ---
    if let Some(file) = extract_pattern(&lower, &[
        r"blame\s+(\S+)",
        r"who\s+(?:changed|edited|wrote)\s+(\S+)",
    ]) {
        return Some(ShortcutResult::single(
            &format!("Show line-by-line blame for '{file}'"),
            &format!("git blame {file}"),
        ));
    }

    None
}

fn matches_any(input: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|p| input.contains(p))
}

fn extract_pattern(input: &str, patterns: &[&str]) -> Option<String> {
    for pat in patterns {
        if let Ok(re) = Regex::new(pat) {
            if let Some(caps) = re.captures(input) {
                if let Some(m) = caps.get(1) {
                    return Some(m.as_str().to_string());
                }
            }
        }
    }
    None
}
