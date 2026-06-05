use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use dirs;

pub const DEFAULT_MODEL_FAST: &str = "qwen2.5:3b";
pub const DEFAULT_MODEL_SMART: &str = "qwen2.5:3b";
/// Default [mistral.rs](https://github.com/EricLBuehler/mistral.rs) OpenAI-compatible server (`mistralrs serve`).
pub const DEFAULT_ENDPOINT_MISTRALRS: &str = "http://127.0.0.1:1234";
pub const DEFAULT_ENDPOINT_OLLAMA: &str = "http://localhost:11434";
const DEFAULT_KEEP_ALIVE: &str = "10m";

/// Hugging Face model id when the configured name is the Ollama-style `qwen2.5:3b` tag.
pub const HF_QWEN25_3B: &str = "Qwen/Qwen2.5-3B-Instruct";

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    #[default]
    Auto,
    /// OpenAI-compatible HTTP API ([mistral.rs](https://github.com/EricLBuehler/mistral.rs) `serve`, llama.cpp, etc.).
    #[serde(alias = "mistralrs")]
    MistralrsHttp,
    Ollama,
    Openai,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_model_fast")]
    pub model_fast: String,
    #[serde(default = "default_model_smart")]
    pub model_smart: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default = "default_endpoint")]
    pub endpoint: String,
    /// Ollama URL used when `backend = auto` or `backend = ollama`.
    #[serde(default = "default_endpoint_ollama")]
    pub endpoint_ollama: String,
    #[serde(default = "default_keep_alive")]
    pub keep_alive: String,
    #[serde(default)]
    pub backend: Backend,
    #[serde(default)]
    pub aliases: HashMap<String, String>,
}

fn default_model_fast() -> String {
    DEFAULT_MODEL_FAST.to_string()
}

fn default_model_smart() -> String {
    DEFAULT_MODEL_SMART.to_string()
}

fn default_endpoint() -> String {
    DEFAULT_ENDPOINT_MISTRALRS.to_string()
}

fn default_endpoint_ollama() -> String {
    DEFAULT_ENDPOINT_OLLAMA.to_string()
}

fn default_keep_alive() -> String {
    DEFAULT_KEEP_ALIVE.to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            model_fast: default_model_fast(),
            model_smart: default_model_smart(),
            model: None,
            endpoint: default_endpoint(),
            endpoint_ollama: default_endpoint_ollama(),
            keep_alive: default_keep_alive(),
            backend: Backend::default(),
            aliases: HashMap::new(),
        }
    }
}

impl Backend {
    pub fn label(self) -> &'static str {
        match self {
            Backend::Auto => "auto",
            Backend::MistralrsHttp => "mistralrs",
            Backend::Ollama => "ollama",
            Backend::Openai => "openai",
        }
    }
}

/// Map Ollama-style model tags to Hugging Face ids for mistral.rs / OpenAI-compatible servers.
pub fn resolve_model_for_server(model: &str) -> String {
    if model == DEFAULT_MODEL_FAST || model == "qwen2.5:3b" {
        HF_QWEN25_3B.to_string()
    } else {
        model.to_string()
    }
}

const COMPLEX_KEYWORDS: &[&str] = &[
    "rewrite", "rebase", "squash", "cherry-pick", "cherry pick",
    "bisect", "filter", "reflog", "submodule", "subtree",
    "worktree", "every commit", "all commits", "multiple commits",
    "rename commit", "reword", "interactive",
    "conflict", "resolve", "hook", "migrate",
    "convert", "split", "reorganize", "restructure",
    "history", "rewrite history",
    "how many", "how much", "who are", "who has", "which branches",
    "pending", "review", "pull request", "pr ",
    "compare", "between", "since", "contributors", "committers",
    "analyze", "statistics", "stats", "summary",
    "multiple branches", "all branches", "merge all",
];

const PR_KEYWORDS: &[&str] = &[
    "pull request",
    "create a pr",
    "create pr",
    "open a pr",
    "open pr",
    "new pr",
    "merge pr",
    "list pr",
    "show pr",
    " pr ",
    " pr to",
    " pr from",
    " pr for",
];

pub fn is_pr_task(task: &str) -> bool {
    let lower = task.to_lowercase();
    PR_KEYWORDS.iter().any(|k| lower.contains(k)) || lower.ends_with(" pr")
}

pub fn is_complex_task(task: &str) -> bool {
    let lower = task.to_lowercase();
    is_pr_task(&lower) || COMPLEX_KEYWORDS.iter().any(|k| lower.contains(k))
}

impl Config {
    pub fn config_path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".git-cli.toml"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::config_path() else {
            return Self::default();
        };

        match fs::read_to_string(&path) {
            Ok(contents) => toml::from_str(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path().ok_or("Could not determine home directory")?;
        let contents =
            toml::to_string_pretty(self).map_err(|e| format!("Failed to serialize config: {e}"))?;
        fs::write(&path, contents).map_err(|e| format!("Failed to write {}: {e}", path.display()))
    }

    pub fn apply_overrides(mut self, model: Option<String>, endpoint: Option<String>) -> Self {
        if let Some(m) = model {
            self.model = Some(m);
        }
        if let Some(e) = endpoint {
            self.endpoint = e;
        }
        self
    }

    pub fn select_model(&self, task: &str) -> String {
        if let Some(ref m) = self.model {
            return m.clone();
        }
        if is_complex_task(task) {
            self.model_smart.clone()
        } else {
            self.model_fast.clone()
        }
    }

    pub fn resolve_alias(&self, input: &str) -> String {
        self.aliases
            .get(input)
            .cloned()
            .unwrap_or_else(|| input.to_string())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PromptExample {
    pub task: String,
    pub commands: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PromptConfig {
    #[serde(default)]
    pub preamble: Option<String>,
    #[serde(default)]
    pub examples: Vec<PromptExample>,
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            preamble: None,
            examples: Vec::new(),
        }
    }
}

impl PromptConfig {
    pub fn config_dir() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".config").join("git-cli"))
    }

    pub fn config_path() -> Option<PathBuf> {
        Self::config_dir().map(|d| d.join("prompt.toml"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::config_path() else {
            return Self::default();
        };

        match fs::read_to_string(&path) {
            Ok(contents) => toml::from_str(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }
}
