mod cli;
mod config;
mod context;
mod executor;
mod ollama;
mod prompt;


use clap::{CommandFactory, Parser};
use clap_complete::generate;
use cli::{Cli, Commands};
use colored::Colorize;
use config::{Config, PromptConfig};
use context::GitContext;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Config { model, endpoint }) => {
            handle_config(model, endpoint);
            return;
        }
        Some(Commands::Examples) => {
            print_examples();
            return;
        }
        Some(Commands::Completions { shell }) => {
            let mut cmd = Cli::command();
            generate(shell, &mut cmd, "git-cli", &mut std::io::stdout());
            return;
        }
        Some(Commands::InitConfig) => {
            handle_init_config();
            return;
        }
        None => {}
    }

    let task = match cli.task {
        Some(t) => t,
        None => {
            eprintln!(
                "{}: Please provide a task description.\n\n  {} git-cli \"undo my last commit\"\n  {} git-cli examples",
                "Error".red().bold(),
                "Usage:".dimmed(),
                "Try:  ".dimmed()
            );
            std::process::exit(1);
        }
    };

    let config = Config::load().apply_overrides(cli.model, cli.endpoint);
    let resolved_task = config.resolve_alias(&task);

    eprintln!("{} Gathering git context...", "●".cyan());
    let git_context = GitContext::gather();

    if !git_context.is_repo {
        eprintln!(
            "{} Not inside a git repository. Commands will be generated without repo context.",
            "Warning:".yellow().bold()
        );
    }

    let selected_model = config.select_model(&resolved_task);
    let complexity = if config::is_complex_task(&resolved_task) {
        "complex → smart model"
    } else {
        "simple → fast model"
    };

    let system_prompt = prompt::build_system_prompt();
    let user_prompt = prompt::build_user_prompt(&resolved_task, &git_context);

    if cli.verbose {
        eprintln!("\n{}\n{}\n\n{}\n{}\n",
            "── System ──".dimmed(), system_prompt.dimmed(),
            "── User ──".dimmed(), user_prompt.dimmed(),
        );
    }

    eprintln!(
        "{} Asking {} ({}) for git commands...\n",
        "●".cyan(),
        selected_model.bold(),
        complexity.dimmed()
    );

    let response =
        match ollama::generate(&config.endpoint, &selected_model, &system_prompt, &user_prompt, &config.keep_alive)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("\n{} {e}", "Error:".red().bold());
                eprintln!(
                    "\n{} Make sure Ollama is running: {}",
                    "Hint:".yellow().bold(),
                    "ollama serve".dimmed()
                );
                std::process::exit(1);
            }
        };

    let parsed = executor::parse_response(&response);
    executor::display(&parsed);

    if cli.execute {
        if let Err(e) = executor::execute_commands(&parsed, cli.force) {
            eprintln!("{} {e}", "Error:".red().bold());
            std::process::exit(1);
        }
    }
}

fn handle_config(model: Option<String>, endpoint: Option<String>) {
    if model.is_none() && endpoint.is_none() {
        let config = Config::load();
        println!("{}", "Current configuration:".bold());
        println!("  model_fast  = {}", config.model_fast.green());
        println!("  model_smart = {}", config.model_smart.green());
        println!("  endpoint    = {}", config.endpoint.green());
        println!("  keep_alive  = {}", config.keep_alive.green());
        if !config.aliases.is_empty() {
            println!("  {}", "aliases:".bold());
            for (alias, expansion) in &config.aliases {
                println!("    {} → {}", alias.cyan(), expansion.dimmed());
            }
        }
        if let Some(path) = Config::config_path() {
            println!("  file        = {}", path.display().to_string().dimmed());
        }
        return;
    }

    let mut config = Config::load();
    if let Some(m) = model {
        println!("  model → {}", m.green());
        config.model = Some(m);
    }
    if let Some(e) = endpoint {
        println!("  endpoint → {}", e.green());
        config.endpoint = e;
    }

    match config.save() {
        Ok(()) => println!("{}", "Configuration saved.".green().bold()),
        Err(e) => {
            eprintln!("{} {e}", "Error:".red().bold());
            std::process::exit(1);
        }
    }
}

fn print_examples() {
    println!("{}", "git-cli — Example Tasks".bold().underline());
    println!();
    println!("{}", "Basics (fast model):".cyan().bold());
    println!("  git-cli \"show status\"");
    println!("  git-cli \"show diff\"");
    println!("  git-cli \"show recent commits\"");
    println!();
    println!("{}", "Branching (fast model):".cyan().bold());
    println!("  git-cli \"create branch feature/auth\"");
    println!("  git-cli \"switch to main\"");
    println!("  git-cli \"delete branch old-feature\"");
    println!();
    println!("{}", "Staging & Committing (fast model):".cyan().bold());
    println!("  git-cli \"stage all changes\"");
    println!("  git-cli \"amend last commit\"");
    println!("  git-cli \"unstage everything\"");
    println!();
    println!("{}", "History & Undoing (fast model):".cyan().bold());
    println!("  git-cli \"undo my last commit\"");
    println!("  git-cli \"discard all changes\"");
    println!();
    println!("{}", "Advanced (auto-switches to smart model):".cyan().bold());
    println!("  git-cli \"squash last 3 commits\"");
    println!("  git-cli \"rewrite all commit messages to use conventional format\"");
    println!("  git-cli \"cherry-pick commit abc123 onto this branch\"");
    println!("  git-cli \"rebase this branch onto main\"");
    println!();
    println!("{}", "Tips:".dimmed());
    println!("  {} to execute commands after review", "--execute / -x".bold());
    println!("  {} to allow destructive commands", "--force".bold());
    println!("  {} to override model selection", "--model / -m".bold());
}

fn handle_init_config() {
    let Some(dir) = PromptConfig::config_dir() else {
        eprintln!("{} Could not determine home directory.", "Error:".red().bold());
        std::process::exit(1);
    };
    let path = dir.join("prompt.toml");

    if path.exists() {
        eprintln!(
            "{} {} already exists. Edit it directly or delete it to re-scaffold.",
            "Skipped:".yellow().bold(),
            path.display()
        );
        return;
    }

    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("{} Failed to create {}: {e}", "Error:".red().bold(), dir.display());
        std::process::exit(1);
    }

    let starter = r#"# git-cli prompt configuration
# This file lets you customize the system prompt sent to the LLM.
# Location: ~/.config/git-cli/prompt.toml

# ─── Preamble ───────────────────────────────────────────────
# Override the default role/rules preamble. Safety rules (rebase -i
# blocking, PR rules, branch lifecycle) are always appended and
# cannot be overridden.
#
# Uncomment and edit to replace the default preamble:
# preamble = """
# You are a Git command-line expert. Given a task, output ONLY the exact
# git/gh commands needed.
# """

# ─── Custom Examples ────────────────────────────────────────
# Add your own few-shot examples. These are appended AFTER the
# built-in examples, giving the LLM extra patterns to learn from.
#
# Each [[examples]] entry needs a `task` and `commands` field:

# [[examples]]
# task = "tag the current commit as v1.0.0"
# commands = """
# # Create an annotated tag
# git tag -a v1.0.0 -m "Release v1.0.0"
# # Push the tag to remote
# git push origin v1.0.0"""

# [[examples]]
# task = "show the diff between main and develop"
# commands = """
# # Compare main and develop branches
# git diff main..develop"""
"#;

    match std::fs::write(&path, starter) {
        Ok(()) => {
            println!("{} Created {}", "✓".green().bold(), path.display());
            println!(
                "  {} Edit this file to customize the LLM prompt.",
                "Hint:".dimmed()
            );
        }
        Err(e) => {
            eprintln!("{} Failed to write {}: {e}", "Error:".red().bold(), path.display());
            std::process::exit(1);
        }
    }
}
