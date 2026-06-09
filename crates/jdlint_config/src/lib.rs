//! jdlint.json configuration schema and loader.
//!
//! `JdlintConfig` is the contract; every field and its default is
//! documented here. No magic or implicit behavior (Principle 6).

use serde::{Deserialize, Serialize};

/// Top-level jdlint.json configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct JdlintConfig {
    /// Rules to enable. Defaults to all rules.
    pub rules: RulesConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RulesConfig {
    // Rule-specific overrides populated in M4
}

impl JdlintConfig {
    pub fn load(_path: &std::path::Path) -> Result<Self, ConfigError> {
        todo!("config loader — implemented in M3")
    }
}

#[derive(Debug)]
pub struct ConfigError(pub String);
