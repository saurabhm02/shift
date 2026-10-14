use anyhow::{Context, Result};
use std::path::PathBuf;

pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    pub fn from_env() -> Result<Paths> {
        Paths::resolve(
            std::env::var_os("SHIFT_HOME").map(PathBuf::from),
            std::env::var_os("HOME").map(PathBuf::from),
        )
    }

    pub fn resolve(shift_home: Option<PathBuf>, home: Option<PathBuf>) -> Result<Paths> {
        let root = match shift_home {
            Some(dir) => dir,
            None => home
                .context("Home is not set")?
                .join("Library/Application Support/Shift"),
        };
        Ok(Paths { root })
    }

    pub fn config_file(&self) -> PathBuf {
        self.root.join("config.toml")
    }
}
