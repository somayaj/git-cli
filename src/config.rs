use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const DEFAULT_MODEL_FAST: &str = "qwen2.5:3b";
const DEFAULT_MODEL_SMART: &str = "mistral:7b-instruct-q4_0";
const DEFAULT_ENDPOINT: &str = "http://localhost:11434";
const DEFAULT_KEEP_ALIVE: &str = "10m";

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
    #[serde(default = "default_keep_alive")]
    pub keep_alive: String,
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
    DEFAULT_ENDPOINT.to_string()
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
            keep_alive: default_keep_alive(),
            aliases: HashMap::new(),
        }
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

pub fn is_complex_task(task: &str) -> bool {
    let lower = task.to_lowercase();
    COMPLEX_KEYWORDS.iter().any(|k| lower.contains(k))
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
