mod cli;
mod config;
mod context;
mod executor;
mod ollama;
mod prompt;
mod shortcuts;

use clap::{CommandFactory, Parser};
use clap_complete::generate;
use cli::{Cli, Commands};
use colored::Colorize;
use config::Config;
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

    // Try shortcuts first for instant results
    if let Some(shortcut) = shortcuts::try_shortcut(&resolved_task) {
        eprintln!("{} Matched shortcut (instant)", "⚡".cyan());
        let response = shortcut.to_response_string();
        let parsed = executor::parse_response(&response);
        executor::display(&parsed);

        if cli.execute {
            if let Err(e) = executor::execute_commands(&parsed, cli.force) {
                eprintln!("{} {e}", "Error:".red().bold());
                std::process::exit(1);
            }
        }
        return;
    }

    eprintln!("{} Gathering git context...", "●".cyan());
    let git_context = GitContext::gather();

    if !git_context.is_repo {
        eprintln!(
            "{} Not inside a git repository. Commands will be generated without repo context.",
            "Warning:".yellow().bold()
        );
    }

    let full_prompt = prompt::build_prompt(&resolved_task, &git_context);

    if cli.verbose {
        eprintln!("\n{}\n{}\n", "── Prompt ──".dimmed(), full_prompt.dimmed());
    }

    eprintln!(
        "{} Asking {} for git commands...\n",
        "●".cyan(),
        config.model.bold()
    );

    let response =
        match ollama::generate(&config.endpoint, &config.model, &full_prompt, &config.keep_alive)
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
        println!("  model      = {}", config.model.green());
        println!("  endpoint   = {}", config.endpoint.green());
        println!("  keep_alive = {}", config.keep_alive.green());
        if !config.aliases.is_empty() {
            println!("  {}", "aliases:".bold());
            for (alias, expansion) in &config.aliases {
                println!("    {} → {}", alias.cyan(), expansion.dimmed());
            }
        }
        if let Some(path) = Config::config_path() {
            println!("  file       = {}", path.display().to_string().dimmed());
        }
        return;
    }

    let mut config = Config::load();
    if let Some(m) = model {
        println!("  model    → {}", m.green());
        config.model = m;
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
    println!("{}", "Basics:".cyan().bold());
    println!("  git-cli \"show status\"");
    println!("  git-cli \"show diff\"");
    println!("  git-cli \"show recent commits\"");
    println!();
    println!("{}", "Branching:".cyan().bold());
    println!("  git-cli \"create branch feature/auth\"");
    println!("  git-cli \"switch to main\"");
    println!("  git-cli \"delete branch old-feature\"");
    println!("  git-cli \"list all branches\"");
    println!();
    println!("{}", "Staging & Committing:".cyan().bold());
    println!("  git-cli \"stage all changes\"");
    println!("  git-cli \"amend last commit\"");
    println!("  git-cli \"unstage everything\"");
    println!();
    println!("{}", "History & Undoing:".cyan().bold());
    println!("  git-cli \"undo my last commit\"");
    println!("  git-cli \"squash last 3 commits\"");
    println!("  git-cli \"discard all changes\"");
    println!();
    println!("{}", "Remote:".cyan().bold());
    println!("  git-cli \"pull latest changes\"");
    println!("  git-cli \"push and set upstream\"");
    println!("  git-cli \"fetch all remotes\"");
    println!();
    println!("{}", "Stash:".cyan().bold());
    println!("  git-cli \"stash changes\"");
    println!("  git-cli \"pop stash\"");
    println!("  git-cli \"list stashes\"");
    println!();
    println!("{}", "Advanced (uses LLM):".cyan().bold());
    println!("  git-cli \"cherry-pick commit abc123 onto this branch\"");
    println!("  git-cli \"rebase this branch onto main resolving conflicts\"");
    println!("  git-cli \"set up git hooks for pre-commit linting\"");
    println!();
    println!("{}", "Tips:".dimmed());
    println!("  {} to execute commands after review", "--execute / -x".bold());
    println!("  {} to allow destructive commands", "--force".bold());
    println!(
        "  {} to define shortcuts in ~/.git-cli.toml",
        "aliases".bold()
    );
}
