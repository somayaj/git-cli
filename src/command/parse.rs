use super::gh;
use super::git;
use super::quote::{quotes_balanced, shell_split, unquoted_contains_any};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCommand {
    pub raw: String,
    pub action: PlannedAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannedAction {
    Git(GitAction),
    Gh(GhAction),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitAction {
    Status,
    Add { paths: Vec<String> },
    Commit { message: String, amend: bool },
    Checkout { branch: String, create: bool },
    Push {
        remote: String,
        branch: String,
        force: bool,
    },
    PushDeleteRemote { remote: String, branch: String },
    Log,
    Reset {
        mode: ResetMode,
        target: String,
    },
    BranchDelete { name: String },
    Clean { force: bool, extra: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResetMode {
    Soft,
    Mixed,
    Hard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GhAction {
    PrCreate {
        base: String,
        head: String,
        title: String,
        body: String,
    },
    PrMerge {
        number: u32,
        merge: bool,
        delete_branch: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockReason {
    UnbalancedQuotes,
    InjectionPattern,
    InteractiveRebase,
    InvalidRefspec,
    InvalidHeadOffset,
    MalformedCommit,
    UnknownSubcommand,
    MissingRequiredFlag,
    InvalidGhSubcommand,
}

const INJECTION_PATTERNS: &[&str] = &["&&", "||", ";", "$(", "`", "|"];

pub fn try_parse_command(cmd: &str) -> Result<PlannedAction, BlockReason> {
    if !cmd.starts_with("git ") && !cmd.starts_with("gh ") {
        return Err(BlockReason::UnknownSubcommand);
    }

    if !quotes_balanced(cmd) {
        return Err(BlockReason::UnbalancedQuotes);
    }

    if unquoted_contains_any(cmd, INJECTION_PATTERNS) {
        return Err(BlockReason::InjectionPattern);
    }

    if cmd.contains("rebase -i") || cmd.contains("rebase --interactive") {
        return Err(BlockReason::InteractiveRebase);
    }

    let parts = shell_split(cmd);
    if parts.is_empty() {
        return Err(BlockReason::UnknownSubcommand);
    }

    match parts[0].as_str() {
        "git" => git::parse_git(&parts, cmd),
        "gh" => gh::parse_gh(&parts),
        _ => Err(BlockReason::UnknownSubcommand),
    }
}
