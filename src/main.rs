use anyhow::Result;
use clap::{Paerser, Subcommand};
use std::path::PathBuf;

mod app;
mod git;
mod ui;

use app::App;

#[derive(Parser)]
#[command(name = "git-hydra")]
#[command(about = "a tui git interface inspired by GitKraken.")]

struct Cli{
    // repo path (defaults to the current dir.)
    #[arg(short, long)]
    repo: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Ui,
    Status,
    Log {
        #[arg(short, long, default_value = "10")]
        count: usize,
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let repo_path = cli.repo.unwrap_or_else(|| PathBuf::from("."));

    match cli.command {
        
    }
}