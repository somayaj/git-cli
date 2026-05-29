mod flags;
mod gh;
mod git;
pub mod parse;
pub mod quote;
pub mod run;

pub use flags::{
    extract_bad_flag, extract_head_branch, extract_pr_merge_number, fix_gh_pr_create_head,
    parse_pr_number_from_output, remove_flag,
};
pub use run::{action_argv, action_display, Executable};

pub use parse::{try_parse_command, BlockReason, GhAction, GitAction, ParsedCommand, PlannedAction};
pub use quote::{quotes_balanced, shell_split, strip_quoted_sections, QuoteAwareChars};

impl PlannedAction {
    pub fn is_destructive(&self) -> bool {
        match self {
            PlannedAction::Git(git) => git.is_destructive(),
            PlannedAction::Gh(_) => false,
        }
    }
}

impl GitAction {
    pub fn is_destructive(&self) -> bool {
        match self {
            GitAction::Push { force, .. } => *force,
            GitAction::Reset { mode, .. } => matches!(mode, parse::ResetMode::Hard),
            GitAction::BranchDelete { .. } => true,
            GitAction::Clean { force, .. } => *force,
            _ => false,
        }
    }
}

impl ParsedCommand {
    pub fn new(raw: String, action: PlannedAction) -> Self {
        Self { raw, action }
    }
}

impl BlockReason {
    pub fn as_blocked_label(&self) -> &'static str {
        match self {
            BlockReason::UnbalancedQuotes => "unbalanced quotes",
            BlockReason::InjectionPattern => "shell injection",
            BlockReason::InteractiveRebase => "interactive rebase",
            BlockReason::InvalidRefspec => "invalid refspec",
            BlockReason::InvalidHeadOffset => "HEAD~n out of range",
            BlockReason::MalformedCommit => "malformed commit",
            BlockReason::UnknownSubcommand => "unknown subcommand",
            BlockReason::MissingRequiredFlag => "missing required flag",
            BlockReason::InvalidGhSubcommand => "unsupported gh command",
        }
    }
}
