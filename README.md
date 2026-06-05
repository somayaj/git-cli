# git-cli

A CLI tool that translates natural-language task descriptions into git (and GitHub CLI) commands using a local LLM — [mistral.rs](https://github.com/EricLBuehler/mistral.rs) (default), [Ollama](https://ollama.com), or [llama.cpp](https://github.com/ggml-org/llama.cpp) (`llama-server`).

Works in any terminal — IntelliJ, Cursor, VS Code, and others.

## Features

- **Dual model routing** — simple tasks use a fast small model, complex tasks auto-switch to a smarter model
- **GitHub CLI support** — built-in shortcut for `create a pr` runs `gh pr create` directly; complex PR workflows use the LLM
- **Safety checks** — destructive commands are blocked unless `--force` is passed; shell injection is always blocked
- **Smart parsing** — handles multi-line LLM output, strips markdown, auto-fixes case/esac glob patterns

## Prerequisites

- [Git](https://git-scm.com) on your PATH
- A local LLM server — [mistral.rs](https://github.com/EricLBuehler/mistral.rs), [Ollama](https://ollama.com), or [llama.cpp](https://github.com/ggml-org/llama.cpp) (`llama-server`)
- [GitHub CLI](https://cli.github.com) (`gh`) — required for PR create/merge tasks

```bash
# macOS
brew install gh
gh auth login

# Quick start with mistral.rs — see "mistral.rs integration" under Configuration
# Or use Ollama: ollama pull qwen2.5:3b && ollama serve

git-cli doctor
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

git-cli uses **`gh` directly** for simple PR creation — no LLM guessing.

```bash
# On a feature branch (not main):
git checkout -b feature/my-change
git-cli "create a pr" --execute          # built-in shortcut → git push + gh pr create
git-cli "create a pr to develop" --execute

# Complex multi-target PR workflows still use the LLM
git-cli "create PRs to v15 and main and merge them" --execute
```

You need the [GitHub CLI](https://cli.github.com) installed and authenticated:

```bash
brew install gh      # macOS
gh auth login
git-cli doctor       # verify git, gh, and your LLM server are ready
```

**Why you must be on a feature branch:** GitHub cannot open a PR from `main` to `main`. If you are on `main`, create a branch first:

```bash
git checkout -b feature/my-change
git push origin feature/my-change
git-cli "create a pr" --execute
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

# Set a custom LLM endpoint
git-cli config --endpoint http://myserver:11434
```

Example `~/.git-cli.toml`:

```toml
model_fast = "qwen2.5:3b"
model_smart = "qwen2.5:3b"
endpoint = "http://127.0.0.1:1234"       # mistral.rs serve (default)
endpoint_ollama = "http://localhost:11434"
keep_alive = "10m"
backend = "auto"   # auto | mistralrs | ollama | openai

[aliases]
undo = "undo my last commit but keep changes"
pub = "push and set upstream"
```

CLI flags (`--model`, `--endpoint`) override the config file. `--model` overrides both fast and smart for that invocation.

### mistral.rs integration

git-cli talks to [mistral.rs](https://github.com/EricLBuehler/mistral.rs) over its **OpenAI-compatible** HTTP API (`/v1/chat/completions` on port **1234** by default). No Ollama runtime is required when you use `backend = "mistralrs"`.

#### 1. Install mistral.rs

**macOS (Apple Silicon, recommended):**

```bash
# Full Xcode + Metal toolchain required for GPU builds
sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer
xcodebuild -downloadComponent MetalToolchain
xcrun metal --version   # must succeed

# Install CLI with Metal + Accelerate
cargo install mistralrs-cli --features "metal accelerate"
```

Or use the upstream install script (detects hardware):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/EricLBuehler/mistral.rs/master/install.sh | sh
```

**Linux (NVIDIA):** see [mistral.rs installation](https://ericlbuehler.github.io/mistral.rs/INSTALLATION.html) for `cuda` / `flash-attn` feature flags.

Verify:

```bash
mistralrs doctor
```

#### 2. Start the server

Leave this running in a dedicated terminal:

```bash
mistralrs serve \
  -m Qwen/Qwen2.5-3B-Instruct \
  --isq q4k \
  --max-seq-len 2048
```

| Flag | Why |
|------|-----|
| `-m Qwen/Qwen2.5-3B-Instruct` | Matches git-cli’s default `qwen2.5:3b` mapping |
| `--isq q4k` | In-situ quantization — fits Apple Silicon RAM, faster load |
| `--max-seq-len 2048` | Enough for git-cli prompts without oversized KV cache |

**Custom port** (if 1234 is busy):

```bash
mistralrs serve -m Qwen/Qwen2.5-3B-Instruct --isq q4k --max-seq-len 2048 -p 1235
```

First start downloads weights and may take several minutes. Wait for `Dummy run completed` and the server listening (not `Address already in use`).

**Check the server:**

```bash
curl -s http://127.0.0.1:1234/v1/models
```

#### 3. Configure git-cli

Default settings (used when `~/.git-cli.toml` is missing):

| Setting | Default |
|---------|---------|
| `endpoint` | `http://127.0.0.1:1234` |
| `endpoint_ollama` | `http://localhost:11434` |
| `backend` | `auto` |
| `model_fast` / `model_smart` | `qwen2.5:3b` |

**mistral.rs only** — add to `~/.git-cli.toml`:

```toml
model_fast = "qwen2.5:3b"
model_smart = "qwen2.5:3b"
endpoint = "http://127.0.0.1:1234"
endpoint_ollama = "http://localhost:11434"
backend = "mistralrs"
```

**Custom port:**

```toml
endpoint = "http://127.0.0.1:1235"
backend = "mistralrs"
```

**Auto (default)** — Ollama wins if it is running on `endpoint_ollama`; otherwise git-cli uses mistral.rs on `endpoint`:

```toml
backend = "auto"
endpoint = "http://127.0.0.1:1234"
endpoint_ollama = "http://localhost:11434"
```

Stop Ollama when you want git-cli to use mistral.rs under `auto`:

```bash
brew services stop ollama   # macOS
```

**Ollama only** (ignore mistral.rs):

```toml
endpoint = "http://localhost:11434"
endpoint_ollama = "http://localhost:11434"
backend = "ollama"
```

#### 4. Model names

git-cli sends the model id from config to the server. For mistral.rs, Ollama-style tags are mapped automatically:

| In `~/.git-cli.toml` | Sent to mistral.rs |
|----------------------|-------------------|
| `qwen2.5:3b` | `Qwen/Qwen2.5-3B-Instruct` |
| `Qwen/Qwen2.5-3B-Instruct` | unchanged |
| any other string | unchanged |

The Hugging Face id in `mistralrs serve -m` must match what git-cli requests (or use `qwen2.5:3b` in config).

#### 5. Run git-cli

```bash
git-cli doctor
git-cli "show status"
git-cli "show status" --verbose   # stream raw LLM output on stderr
```

`git-cli doctor` should report something like: `mistral.rs server at http://127.0.0.1:1234`.

Override for one command:

```bash
git-cli "show status" --endpoint http://127.0.0.1:1235 --model qwen2.5:3b
```

#### 6. Troubleshooting

| Problem | Fix |
|---------|-----|
| `No LLM server reachable` | Start `mistralrs serve`; check `curl http://127.0.0.1:1234/v1/models` |
| `Address already in use` | Another `mistralrs serve` is running — use it or `kill` the old PID |
| `model does not fit on devices` | Add `--isq q4k`, lower `--max-seq-len 2048`, free RAM, or use a smaller model |
| `xcrun: unable to find utility "metal"` | Install Xcode + Metal toolchain (see step 1) |
| git-cli uses Ollama, not mistral.rs | `backend = "mistralrs"` or stop Ollama when using `auto` |
| Very slow first request | Large git-cli prompt + cold server; second request is faster (prefix cache) |
| Wrong / empty commands | Use `qwen2.5:3b` or `Qwen2.5-3B`; avoid tiny models (0.5B) for git-cli |

**Performance:** On macOS, Ollama with `qwen2.5:3b` is often faster for day-to-day git-cli use. mistral.rs is a good fit when you want Hugging Face models, in-situ quantization, or a single Rust inference stack.

See also: [mistral.rs docs](https://github.com/EricLBuehler/mistral.rs), [device mapping](https://github.com/EricLBuehler/mistral.rs/blob/master/docs/DEVICE_MAPPING.md).

### llama.cpp (llama-server)

git-cli auto-detects llama.cpp when Ollama is not running on `endpoint_ollama`. Use the model name from `--alias`:

```bash
llama-server -hf unsloth/Qwen3-4B-GGUF:Q4_K_M --alias qwen3-4b --port 11434

git-cli config --model qwen3-4b
git-cli config --endpoint http://localhost:11434
git-cli doctor   # ✓ llm — OpenAI-compatible server (llama.cpp) at ...
git-cli "show status"
```

Set `backend = "openai"` in `~/.git-cli.toml` to skip Ollama detection and always use the OpenAI-compatible API.

### Prompt Customization

You can customize the system prompt sent to the LLM via `~/.config/git-cli/prompt.toml`. Scaffold a starter file:

```bash
git-cli init-config
```

The prompt is built in three layers:

| Layer | Location | Overridable? |
|-------|----------|-------------|
| **Preamble** | `prompt.toml` → `preamble` | Yes — replace the default role/rules text |
| **Safety rules** | Embedded in binary | No — always appended (PR rules, `rebase -i` blocking, branch lifecycle) |
| **Examples** | `prompt.toml` → `[[examples]]` | Additive — your examples are appended after the built-in ones |

Example `~/.config/git-cli/prompt.toml`:

```toml
# Replace the default preamble (optional)
# preamble = "You are a senior DevOps engineer..."

# Add custom few-shot examples
[[examples]]
task = "tag the current commit as v1.0.0"
commands = """
# Create an annotated tag
git tag -a v1.0.0 -m "Release v1.0.0"
# Push the tag to remote
git push origin v1.0.0"""

[[examples]]
task = "show the diff between main and develop"
commands = """
# Compare main and develop branches
git diff main..develop"""
```

If the file doesn't exist, the built-in defaults are used — no configuration required.

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
5. Sends the prompt to the local LLM (Ollama `/api/chat` or OpenAI-compatible `/v1/chat/completions` for mistral.rs / llama.cpp).
6. Parses the response: joins multi-line commands, strips markdown, auto-fixes case/esac globs.
7. Validates commands: blocks injection, checks HEAD~N against actual commit count, flags destructive ops.
8. Displays commands with syntax highlighting.
9. With `--execute`, runs each command sequentially.

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for the full guide, including:

- Local setup, testing, and project layout
- Branch naming (`feat/`, `fix/`, `docs/`, etc.) and conventional commit messages
- Pull request requirements and review process
- Merge policy (squash merge to `main`, releases via version tags)

Quick start for contributors:

```bash
git clone https://github.com/somayaj/git-cli.git
cd git-cli
cargo test
```

## License

MIT
