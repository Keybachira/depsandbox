use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const CONFIG_FILE: &str = "depsandbox.toml";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    /// Ecossistema fixado (senão é detectado).
    pub ecosystem: Option<String>,
    #[serde(default)]
    pub checks: ChecksConfig,
    #[serde(default)]
    pub ignore: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChecksConfig {
    pub test: Option<String>,
    pub build: Option<String>,
    pub typecheck: Option<String>,
}

impl Config {
    pub fn load(dir: &Path) -> Result<Self> {
        let p = dir.join(CONFIG_FILE);
        if !p.exists() {
            return Ok(Self::default());
        }
        let s = std::fs::read_to_string(&p).with_context(|| format!("a ler {}", p.display()))?;
        toml::from_str(&s).with_context(|| format!("a interpretar {}", p.display()))
    }

    pub fn to_toml(&self) -> Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }

    pub fn save(&self, dir: &Path) -> Result<()> {
        std::fs::write(dir.join(CONFIG_FILE), self.to_toml()?)?;
        Ok(())
    }
}
