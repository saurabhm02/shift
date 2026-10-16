pub mod cli;
pub mod config;
pub mod path;
pub mod post;
pub mod read;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command, ConfigCommand};
use config::Config;
use path::Paths;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let paths = Paths::from_env()?;

    match cli.command {
        Command::Config(ConfigCommand::Show) => {
            let config = Config::load(&paths.config_file())?;
            print!("{}", toml::to_string_pretty(&config)?);
        }
        Command::Config(ConfigCommand::Path) => {
            println!("{}", paths.config_file().display());
        }
        Command::Query(words) => {
            println!("search for: {}", words.join(" "));
        }
        Command::Insta
        | Command::Files
        | Command::Serve
        | Command::Stop
        | Command::Status
        | Command::Stdio
        | Command::Bench
        | Command::Install
        | Command::Uninstall => {
            println!("not built yet");
        }
    }
    Ok(())
}
