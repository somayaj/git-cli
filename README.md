# git-cli

A CLI tool that translates natural-language task descriptions into git commands using a local [Ollama](https://ollama.com) LLM.

Works in any terminal — including IntelliJ, Cursor, VS Code, and others.

Common tasks like "undo last commit" or "stash changes" are matched instantly without calling the LLM at all.

## Prerequisites

- [Rust](https://rustup.rs) (for building from source)
- [Ollama](https://ollama.com) running locally with a model pulled (default: `mistral`)

```bash
# Install Ollama, then pull a fast model
ollama pull qwen2.5:1.5b
```

## Installation

```bash
cargo install --path .
```

## Usage

```bash
# Describe what you want to do — git-cli prints the commands
git-cli "undo my last commit but keep the changes"

# Add --execute (-x) to run the commands immediately
git-cli "show status" --execute

# Override the model for a single invocation
git-cli "create a branch called feature/auth" --model codellama

# Show the full prompt sent to the LLM
git-cli "rebase onto main" --verbose

# See all example tasks
git-cli examples
```

## Instant Shortcuts

~30 common tasks are recognized by keyword and return instantly — no LLM needed:

```bash
git-cli "show status"           # instant
git-cli "undo my last commit"   # instant
git-cli "stash changes"         # instant
git-cli "create branch foo"     # instant
git-cli "list all branches"     # instant
git-cli "pull latest"           # instant
```

Anything not matched falls through to the LLM for a full answer.

## Safety

Destructive commands (`push --force`, `reset --hard`, `clean -f`, `branch -D`) are highlighted in red and blocked from execution unless you pass `--force`:

```bash
git-cli "force push to origin" --execute          # blocked
git-cli "force push to origin" --execute --force   # allowed
```

Commands containing shell injection patterns (`&&`, `|`, `;`, `$()`) are always blocked.

## Configuration

Persist defaults in `~/.git-cli.toml`:

```bash
# View current settings
git-cli config

# Set the default model
git-cli config --model qwen2.5:1.5b

# Set a custom Ollama endpoint
git-cli config --endpoint http://myserver:11434
```

CLI flags (`--model`, `--endpoint`) override the config file.

### Aliases

Define shortcuts in `~/.git-cli.toml` to save typing:

```toml
[aliases]
undo = "undo my last commit but keep changes"
squash3 = "squash last 3 commits"
pub = "push and set upstream"
```

Then run `git-cli undo` instead of the full description.

## Shell Completions

Generate completions for your shell:

```bash
# Bash
git-cli completions bash > ~/.bash_completions/git-cli
source ~/.bash_completions/git-cli

# Zsh
git-cli completions zsh > ~/.zfunc/_git-cli

# Fish
git-cli completions fish > ~/.config/fish/completions/git-cli.fish
```

## How It Works

1. Checks for an alias match in your config.
2. Checks for an instant shortcut match (~30 common tasks).
3. If no match, gathers context from the current git repo (branch, status, recent log, remotes).
4. Builds a prompt combining the context with your task description.
5. Sends the prompt to the local Ollama API (with optimized parameters for speed).
6. Parses and validates the response, stripping markdown artifacts.
7. Displays the git commands with syntax highlighting.
8. With `--execute`, runs each command sequentially.

## License

MIT
