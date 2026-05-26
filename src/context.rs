use std::process::Command;

#[derive(Debug)]
pub struct GitContext {
    pub is_repo: bool,
    pub branch: Option<String>,
    pub status: Option<String>,
    pub recent_log: Option<String>,
    pub remotes: Option<String>,
    pub branches: Option<String>,
    pub open_prs: Option<String>,
}

impl GitContext {
    pub fn gather() -> Self {
        let is_repo = run_git(&["rev-parse", "--is-inside-work-tree"])
            .map(|s| s.trim() == "true")
            .unwrap_or(false);

        if !is_repo {
            return Self {
                is_repo: false,
                branch: None,
                status: None,
                recent_log: None,
                remotes: None,
                branches: None,
                open_prs: None,
            };
        }

        let branch = run_git(&["rev-parse", "--abbrev-ref", "HEAD"]).map(|s| s.trim().to_string());

        let all_prs = run_cmd("gh", &["pr", "list", "--state", "open", "--limit", "20", "--json", "number,title,headRefName,baseRefName", "--template", "{{range .}}#{{.number}} {{.headRefName}} → {{.baseRefName}} \"{{.title}}\"\n{{end}}"]);

        let open_prs = match (&branch, &all_prs) {
            (Some(current_branch), Some(prs)) => {
                let mut mine = Vec::new();
                let mut others = Vec::new();
                for line in prs.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() { continue; }
                    if trimmed.contains(&format!(" {} → ", current_branch)) {
                        mine.push(trimmed.to_string());
                    } else {
                        others.push(trimmed.to_string());
                    }
                }
                let mut result = String::new();
                if !mine.is_empty() {
                    result.push_str(&format!("PRs for current branch ({}):\n", current_branch));
                    for pr in &mine {
                        result.push_str(&format!("  {}\n", pr));
                    }
                }
                if !others.is_empty() {
                    result.push_str("PRs for other branches (DO NOT merge these):\n");
                    for pr in &others {
                        result.push_str(&format!("  {}\n", pr));
                    }
                }
                if result.is_empty() { None } else { Some(result) }
            }
            _ => all_prs,
        };

        Self {
            is_repo: true,
            branch,
            status: run_git(&["status", "--porcelain"]),
            recent_log: run_git(&["log", "--oneline", "-10"]),
            remotes: run_git(&["remote", "-v"]),
            branches: run_git(&["branch", "-a", "--no-color"]),
            open_prs,
        }
    }

    pub fn summary(&self) -> String {
        if !self.is_repo {
            return "Not inside a git repository.".to_string();
        }

        let mut parts = Vec::new();

        if let Some(ref branch) = self.branch {
            parts.push(format!("Current branch: {branch}"));
        }

        if let Some(ref status) = self.status {
            if status.trim().is_empty() {
                parts.push("Working tree: clean".to_string());
            } else {
                parts.push(format!("Working tree status:\n{status}"));
            }
        }

        if let Some(ref log) = self.recent_log {
            if !log.trim().is_empty() {
                parts.push(format!("Recent commits:\n{log}"));
            }
        }

        if let Some(ref remotes) = self.remotes {
            if !remotes.trim().is_empty() {
                parts.push(format!("Remotes:\n{remotes}"));
            }
        }

        if let Some(ref branches) = self.branches {
            if !branches.trim().is_empty() {
                parts.push(format!("All branches:\n{branches}"));
            }
        }

        if let Some(ref prs) = self.open_prs {
            if !prs.trim().is_empty() {
                parts.push(format!("Open PRs:\n{prs}"));
            }
        }

        parts.join("\n\n")
    }
}

fn run_git(args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
}

fn run_cmd(cmd: &str, args: &[&str]) -> Option<String> {
    Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
}
