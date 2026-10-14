use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub ollama_url: String,
    pub embed_model: String,
    pub whisper_model: String,
    pub max_file_chunks: usize,
    pub exclude: Vec<String>,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            ollama_url: "http://localhost:11434".to_string(),
            embed_model: "bge-m3".to_string(),
            whisper_model: String::new(),
            max_file_chunks: 100_000,
            exclude: [
                ".git",
                "node_modules",
                "target",
                "Library/Caches",
                "Library/Mobile Documents",
                "Library/CloudStorage",
                ".Trash",
            ]
            .iter()
            .map(|name| name.to_string())
            .collect(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Config> {
        if !path.exists() {
            return Ok(Config::default());
        }
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }
}
