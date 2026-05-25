use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const DEFAULT_MODEL: &str = "mistral";
const DEFAULT_ENDPOINT: &str = "http://localhost:11434";
const DEFAULT_KEEP_ALIVE: &str = "10m";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_endpoint")]
    pub endpoint: String,
    #[serde(default = "default_keep_alive")]
    pub keep_alive: String,
    #[serde(default)]
    pub aliases: HashMap<String, String>,
}

fn default_model() -> String {
    DEFAULT_MODEL.to_string()
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
            model: default_model(),
            endpoint: default_endpoint(),
            keep_alive: default_keep_alive(),
            aliases: HashMap::new(),
        }
    }
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
            self.model = m;
        }
        if let Some(e) = endpoint {
            self.endpoint = e;
        }
        self
    }

    pub fn resolve_alias(&self, input: &str) -> String {
        self.aliases
            .get(input)
            .cloned()
            .unwrap_or_else(|| input.to_string())
    }
}
