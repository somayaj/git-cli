use std::process::Command;

#[derive(Debug)]
pub struct GitContext {
    pub is_repo: bool,
    pub branch: Option<String>,
    pub status: Option<String>,
    pub recent_log: Option<String>,
    pub remotes: Option<String>,
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
            };
        }

        Self {
            is_repo: true,
            branch: run_git(&["rev-parse", "--abbrev-ref", "HEAD"]).map(|s| s.trim().to_string()),
            status: run_git(&["status", "--porcelain"]),
            recent_log: run_git(&["log", "--oneline", "-10"]),
            remotes: run_git(&["remote", "-v"]),
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
