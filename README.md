# git-cli

A CLI tool that translates natural-language task descriptions into git (and GitHub CLI) commands using a local [Ollama](https://ollama.com) LLM.

Works in any terminal — IntelliJ, Cursor, VS Code, and others.

## Features

- **Dual model routing** — simple tasks use a fast small model, complex tasks auto-switch to a smarter model
- **GitHub CLI support** — generates `gh` commands for PRs, merges, and repo operations
- **Safety checks** — destructive commands are blocked unless `--force` is passed; shell injection is always blocked
- **Smart parsing** — handles multi-line LLM output, strips markdown, auto-fixes case/esac glob patterns

## Prerequisites

- [Ollama](https://ollama.com) running locally with models pulled

```bash
# Pull the default models
ollama pull qwen2.5:3b
```

## Installation

```bash
# From crates.io
cargo install git-cli

# From source
cargo install --path .
```

## Usage

```bash
# Describe what you want — git-cli prints the commands
git-cli "undo my last commit but keep the changes"

# Add --execute (-x) to run the commands immediately
git-cli "show status" --execute

# Use --force for destructive commands
git-cli "force push to origin" --execute --force

# Override the model for a single invocation
git-cli "create a branch called feature/auth" --model mistral

# Show the full prompt sent to the LLM
git-cli "rebase onto main" --verbose

# See all example tasks
git-cli examples
```

## Smart Model Routing

Tasks are automatically classified as simple or complex:

```bash
# Simple tasks → fast model (qwen2.5:3b)
git-cli "show status"
git-cli "create branch feature/auth"
git-cli "stage all changes"

# Complex tasks → smart model (auto-detected)
git-cli "rewrite all commit messages to use conventional format"
git-cli "cherry-pick commit abc123 onto this branch"
git-cli "squash last 3 commits"
```

The output shows which model was selected:

```
● Asking qwen2.5:3b (simple → fast model) for git commands...
● Asking qwen2.5:3b (complex → smart model) for git commands...
```

## GitHub CLI Support

git-cli can also generate `gh` commands for GitHub operations:

```bash
git-cli "create a PR from this branch to main" --execute
git-cli "merge PR #3 and delete the branch" --execute
git-cli "list open pull requests" --execute
```

## Safety

Destructive commands (`push --force`, `reset --hard`, `clean -f`, `branch -D`, `filter-branch`) are highlighted in red and blocked unless you pass `--force`:

```bash
git-cli "force push to origin" --execute          # blocked
git-cli "force push to origin" --execute --force   # allowed
```

Commands containing shell injection patterns (`&&`, `|`, `;`, `$()`) outside of quoted strings are always blocked.

## Configuration

Settings are stored in `~/.git-cli.toml`:

```bash
# View current settings
git-cli config

# Set a custom Ollama endpoint
git-cli config --endpoint http://myserver:11434
```

Example `~/.git-cli.toml`:

```toml
model_fast = "qwen2.5:3b"
model_smart = "qwen2.5:3b"
endpoint = "http://localhost:11434"
keep_alive = "10m"

[aliases]
undo = "undo my last commit but keep changes"
pub = "push and set upstream"
```

CLI flags (`--model`, `--endpoint`) override the config file. `--model` overrides both fast and smart for that invocation.

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
2. Gathers context from the current git repo (branch, status, recent log, remotes).
3. Classifies the task as simple or complex and selects the appropriate model.
4. Builds a system prompt with rules and few-shot examples, plus a user prompt with repo context.
5. Sends the prompt to the local Ollama chat API.
6. Parses the response: joins multi-line commands, strips markdown, auto-fixes case/esac globs.
7. Validates commands: blocks injection, checks HEAD~N against actual commit count, flags destructive ops.
8. Displays commands with syntax highlighting.
9. With `--execute`, runs each command sequentially.

## License

MIT
