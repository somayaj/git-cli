use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "git-cli", version, about = "Translate natural-language task descriptions into git commands")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// The task description in natural language (e.g. "undo my last commit")
    #[arg(value_name = "TASK")]
    pub task: Option<String>,

    /// Execute the generated commands after displaying them
    #[arg(short = 'x', long)]
    pub execute: bool,

    /// Allow execution of destructive commands (push --force, reset --hard, etc.)
    #[arg(long)]
    pub force: bool,

    /// Override the Ollama model to use
    #[arg(short, long)]
    pub model: Option<String>,

    /// Override the Ollama API endpoint
    #[arg(short, long)]
    pub endpoint: Option<String>,

    /// Show the full prompt sent to the LLM
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Persist default settings to ~/.git-cli.toml
    Config {
        /// Set the default model (e.g. mistral, codellama, llama3)
        #[arg(long)]
        model: Option<String>,

        /// Set the default Ollama endpoint URL
        #[arg(long)]
        endpoint: Option<String>,
    },

    /// Show example tasks you can run
    Examples,

    /// Generate shell completions for your shell
    Completions {
        /// Shell to generate completions for (bash, zsh, fish, powershell)
        #[arg(value_name = "SHELL")]
        shell: clap_complete::Shell,
    },
}
