use colored::Colorize;
use std::process::Command;

use crate::config::Backend;
use crate::llm;

pub struct Check {
    pub name: &'static str,
    pub ok: bool,
    pub detail: String,
    pub hint: Option<&'static str>,
}

pub fn gh_on_path() -> bool {
    Command::new("gh")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn check_git() -> Check {
    match Command::new("git").arg("--version").output() {
        Ok(o) if o.status.success() => {
            let version = String::from_utf8_lossy(&o.stdout).trim().to_string();
            Check {
                name: "git",
                ok: true,
                detail: version,
                hint: None,
            }
        }
        Ok(o) => Check {
            name: "git",
            ok: false,
            detail: String::from_utf8_lossy(&o.stderr).trim().to_string(),
            hint: Some("Install git: https://git-scm.com/downloads"),
        },
        Err(e) => Check {
            name: "git",
            ok: false,
            detail: e.to_string(),
            hint: Some("Install git: https://git-scm.com/downloads"),
        },
    }
}

pub fn check_gh() -> Check {
    match Command::new("gh").arg("--version").output() {
        Ok(o) if o.status.success() => {
            let version = String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .unwrap_or("gh")
                .to_string();
            Check {
                name: "gh",
                ok: true,
                detail: version,
                hint: None,
            }
        }
        Ok(o) => Check {
            name: "gh",
            ok: false,
            detail: String::from_utf8_lossy(&o.stderr).trim().to_string(),
            hint: Some("Install GitHub CLI: https://cli.github.com — then run `gh auth login`"),
        },
        Err(_) => Check {
            name: "gh",
            ok: false,
            detail: "not found on PATH".to_string(),
            hint: Some("Install GitHub CLI: brew install gh — then run `gh auth login`"),
        },
    }
}

pub async fn check_llm(endpoint: &str, endpoint_ollama: &str, backend: Backend) -> Check {
    match llm::detect(endpoint, endpoint_ollama, backend).await {
        Ok(status) => Check {
            name: "llm",
            ok: true,
            detail: status.detail,
            hint: None,
        },
        Err(detail) => Check {
            name: "llm",
            ok: false,
            detail,
            hint: Some(
                "Start Ollama (`ollama serve`) or mistral.rs (`mistralrs serve -m Qwen/Qwen2.5-3B-Instruct`)",
            ),
        },
    }
}

pub fn gh_pr_list_error() -> Option<String> {
    if !gh_on_path() {
        return Some(
            "GitHub CLI (gh) not found — PR context unavailable. Install: https://cli.github.com"
                .to_string(),
        );
    }

    let output = Command::new("gh")
        .args([
            "pr",
            "list",
            "--state",
            "open",
            "--limit",
            "1",
            "--json",
            "number",
        ])
        .output()
        .ok()?;

    if output.status.success() {
        return None;
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.contains("auth") || stderr.contains("login") {
        Some(format!(
            "gh not authenticated — PR context unavailable. Run: gh auth login ({stderr})"
        ))
    } else if stderr.contains("not a git repository") {
        None
    } else {
        Some(format!("gh pr list failed — PR context unavailable: {stderr}"))
    }
}

pub async fn run(endpoint: &str, endpoint_ollama: &str, backend: Backend) -> bool {
    println!("{}", "git-cli doctor".bold().underline());
    println!();

    let checks = [
        check_git(),
        check_gh(),
        check_llm(endpoint, endpoint_ollama, backend).await,
    ];

    let mut all_ok = true;
    for check in &checks {
        let icon = if check.ok { "✓".green() } else { "✗".red() };
        println!("  {} {} — {}", icon, check.name.bold(), check.detail);
        if let Some(hint) = check.hint {
            println!("      {} {}", "hint:".dimmed(), hint.dimmed());
        }
        if !check.ok {
            all_ok = false;
        }
    }

    println!();
    if all_ok {
        println!("{}", "All checks passed.".green().bold());
    } else {
        println!("{}", "Some checks failed.".red().bold());
    }

    all_ok
}
