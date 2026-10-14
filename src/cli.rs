use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "shift", about = "Search local files and saved Instagram posts")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Insta,
    Files,
    Serve,
    Stop,
    Status,
    Stdio,
    Bench,
    Install,
    Uninstall,

    #[command(subcommand)]
    Config(ConfigCommand),

    #[command(external_subcommand)]
    Query(Vec<String>),
}

#[derive(Subcommand)]
pub enum ConfigCommand {
    Show,
    Path,
}
