//! falcon.json configuration schema and loader.
//!
//! `FalconConfig` is the contract; every field and its default is
//! documented here. No magic or implicit behavior (Principle 6).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Top-level falcon.json configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FalconConfig {
    /// Rules to enable and configure. Defaults to empty map.
    pub rules: HashMap<String, RuleConfig>,
    /// Patterns to exclude from linting. Defaults to empty vec.
    pub exclude_patterns: Vec<String>,
    /// Per-rule severity overrides (e.g., "warn", "error"). Defaults to empty map.
    pub severity_override: HashMap<String, String>,
    /// Maximum number of errors before stopping. Defaults to None (unlimited).
    pub max_errors: Option<usize>,
}

/// Configuration for a single rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RuleConfig {
    /// Whether this rule is enabled. Defaults to true.
    pub enabled: bool,
    /// Rule-specific options. Defaults to empty map.
    pub options: HashMap<String, serde_json::Value>,
}

impl Default for RuleConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            options: HashMap::new(),
        }
    }
}

/// Error type for configuration loading and parsing.
#[derive(Debug)]
pub struct ConfigError(pub String);

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "config error: {}", self.0)
    }
}

impl std::error::Error for ConfigError {}

/// Load a config from a specific file path.
///
/// # Errors
///
/// Returns `ConfigError` if the file cannot be read or if JSON deserialization fails.
pub fn load_config(path: &Path) -> Result<FalconConfig, ConfigError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| ConfigError(format!("failed to read config file: {}", e)))?;
    serde_json::from_str(&content)
        .map_err(|e| ConfigError(format!("failed to parse config JSON: {}", e)))
}

/// Find a config file starting from `start_dir`, following this priority order:
///
/// 1. `start_dir/falcon.json` — if exists, return it
/// 2. Walk parent dirs up to filesystem root looking for `.git` dir; when found, check `<git_root>/falcon.json`
/// 3. `$HOME/.falcon.json` (via `std::env::var("HOME")`)
/// 4. Return None if nothing found
pub fn find_config(start_dir: &Path) -> Option<PathBuf> {
    // 1. Check start_dir/falcon.json
    let local_config = start_dir.join("falcon.json");
    if local_config.exists() {
        return Some(local_config);
    }

    // 2. Walk parent dirs looking for .git, then check <git_root>/falcon.json
    let mut current = start_dir;
    loop {
        let git_dir = current.join(".git");
        if git_dir.exists() {
            let config_at_git_root = current.join("falcon.json");
            if config_at_git_root.exists() {
                return Some(config_at_git_root);
            }
            break;
        }
        match current.parent() {
            Some(parent) if parent != current => current = parent,
            _ => break,
        }
    }

    // 3. Check $HOME/.falcon.json
    if let Ok(home) = std::env::var("HOME") {
        let home_config = PathBuf::from(home).join(".falcon.json");
        if home_config.exists() {
            return Some(home_config);
        }
    }

    None
}

/// Load config from discovered location, or return default if not found.
///
/// If config file is found but loading fails, logs a warning to stderr and returns default.
pub fn load_or_default(start_dir: &Path) -> FalconConfig {
    match find_config(start_dir) {
        Some(path) => match load_config(&path) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!(
                    "warning: failed to load config from {}: {}",
                    path.display(),
                    e
                );
                FalconConfig::default()
            }
        },
        None => FalconConfig::default(),
    }
}
