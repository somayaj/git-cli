# Contributing to git-cli

Thank you for your interest in contributing! This guide covers how to set up the project locally, run tests, and submit changes.

## Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain)
- [Git](https://git-scm.com)

For manual testing of LLM-related features, you will also need:

- [Ollama](https://ollama.com) running locally with at least one model pulled (e.g. `qwen2.5:3b`)
- [GitHub CLI](https://cli.github.com) (`gh`) for PR-related workflows

## Getting Started

```bash
git clone https://github.com/somayaj/git-cli.git
cd git-cli
cargo build
cargo test
```

Run the CLI from source:

```bash
cargo run -- "show status"
cargo run -- doctor
```

Install locally for day-to-day use:

```bash
cargo install --path .
```

## Project Layout

```
src/
  main.rs       # Entry point
  cli.rs        # Clap argument parsing
  config.rs     # ~/.git-cli.toml loading and task classification
  context.rs    # Git repo context gathering
  executor.rs   # Command execution and safety checks
  ollama.rs     # Ollama API client
  prompt.rs     # System prompt construction
  pr_shortcut.rs # Direct gh pr create shortcut
  doctor.rs     # Environment diagnostics
tests/          # Integration tests
```

## Making Changes

1. Create a branch from `main`:

   ```bash
   git checkout -b feat/my-change
   ```

2. Make your changes and add or update tests when behavior changes.

3. Format and verify before opening a PR:

   ```bash
   cargo fmt
   cargo clippy -- -D warnings
   cargo test
   cargo build --release
   ```

4. Push your branch and open a pull request against `main`.

## Testing

Integration tests live in `tests/` and cover config parsing, prompt configuration, command execution safety, and PR shortcuts. Most tests do not require Ollama or a live git remote.

```bash
# Run all tests
cargo test

# Run a specific test file
cargo test --test config_tests

# Run a single test by name
cargo test pr_task_create_a_pr
```

When adding new behavior, prefer tests that exercise real logic rather than trivial assertions.

## Commit Messages

Use [Conventional Commits](https://www.conventionalcommits.org/) prefixes:

| Prefix       | Use for                          |
|--------------|----------------------------------|
| `feat:`      | New features                     |
| `fix:`       | Bug fixes                        |
| `refactor:`  | Code restructuring, no behavior change |
| `test:`      | Adding or updating tests         |
| `docs:`      | Documentation changes            |
| `chore:`     | Maintenance (deps, CI, tooling)|

Examples:

```
feat: add alias support for squash workflows
fix: block shell injection in quoted strings
docs: document prompt.toml customization
```

Keep the subject line concise. Add a body when the change needs extra context.

## Pull Request Policy

All changes to `main` go through a pull request. Direct pushes to `main` are not allowed.

### Branch Naming

Create a short-lived branch from the latest `main`:

| Prefix       | Use for                          | Example                    |
|--------------|----------------------------------|----------------------------|
| `feat/`      | New features                     | `feat/pr-shortcut-flags`   |
| `fix/`       | Bug fixes                        | `fix/injection-in-quotes`  |
| `docs/`      | Documentation only               | `docs/contributing-guide`  |
| `test/`      | Test additions or fixes          | `test/executor-safety`     |
| `refactor/`  | Code cleanup, no behavior change | `refactor/split-prompt`    |
| `chore/`     | CI, deps, tooling                | `chore/update-clap`        |

### Before Opening a PR

1. Rebase or merge the latest `main` into your branch.
2. Run the full local check:

   ```bash
   cargo fmt
   cargo clippy -- -D warnings
   cargo test
   cargo build --release
   ```

3. Add or update tests when behavior changes.
4. Use conventional commit messages on your branch (see below).

### PR Requirements

Every pull request must meet all of the following:

- **Target branch:** `main`
- **CI green:** the Build workflow must pass (`cargo build` and `cargo test` on Ubuntu)
- **Focused scope:** one logical change per PR — split unrelated fixes into separate PRs
- **Clear description:** explain what changed, why, and how you tested it
- **No secrets:** do not commit tokens, credentials, or local config files
- **Conventional commits:** commit messages use the prefixes defined in this guide

### PR Description Template

Use this structure in your PR body:

```markdown
## Summary
- What changed and why (1–3 bullets)

## Test plan
- [ ] `cargo test`
- [ ] `cargo clippy -- -D warnings`
- [ ] Manual testing (if applicable): describe commands run and results
```

### Review Process

1. Open the PR against `main` and mark it **Draft** if it is still a work in progress.
2. Request review once CI is green and the PR is ready.
3. Address review feedback with new commits on the same branch — do not force-push unless a reviewer asks you to rebase.
4. A maintainer will approve and merge when the PR meets all requirements.

Maintainers may request changes, suggest splitting a PR, or close PRs that are stale, out of scope, or duplicate existing work.

### What We Look For

- Behavior changes include tests
- Error messages and CLI output are clear
- Safety rules (injection blocking, destructive command guards) are preserved
- Documentation is updated when user-facing behavior changes

## Merging Policy

### Who Can Merge

Only repository maintainers merge pull requests.

### Merge Method

PRs are merged using **Squash and merge** by default.

- All commits on the branch are squashed into a single commit on `main`
- The squash commit message must follow conventional commit format
- The PR title is used as the squash commit subject — write it accordingly

Example PR titles:

```
feat: add --dry-run flag to executor
fix: reject unbalanced quotes in parsed commands
docs: add PR and merge policy to CONTRIBUTING.md
```

Use **Rebase and merge** only when a PR contains multiple logically separate commits that must remain distinct in history (rare — discuss with a maintainer first).

Do not use **Create a merge commit** unless explicitly agreed upon for a release or multi-branch integration.

### When a PR Can Be Merged

A PR is eligible to merge when:

1. All required CI checks pass
2. At least one maintainer has approved the review
3. All review threads are resolved
4. The branch is up to date with `main` (rebase or update branch if needed)
5. The change matches the project's scope and coding conventions

### After Merge

- The source branch is deleted after merge
- Maintainers cut releases by pushing a version tag (`v*`), which triggers the Release workflow and publishes to [crates.io](https://crates.io/crates/git-cli)
- Contributors do not need to bump `Cargo.toml` version numbers unless asked — maintainers handle releases

### Hotfixes

For urgent production fixes:

1. Branch from `main` using the `fix/` prefix
2. Keep the change minimal and targeted
3. Mark the PR as a hotfix in the description
4. Maintainers will prioritize review and merge

## Reporting Issues

When filing a bug report, include:

- Your OS and Rust version (`rustc --version`)
- The `git-cli` version or commit SHA
- The exact command you ran and the output you saw
- Whether Ollama and `gh` are installed (run `git-cli doctor` for a quick check)

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](README.md#license).
