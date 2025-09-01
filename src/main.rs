use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::{thread, time::Duration};
use crossterm::{
    execute,
    terminal::{Clear, ClearType},
};
use std::io::{self};

mod app;
mod git;
mod ui;

use app::App;
fn prompt_yes_no(prompt: &str) -> bool {
    use std::io::{self, Write};
    print!("{} ", prompt);
    let _ = io::stdout().flush();
    let mut s = String::new();
    if io::stdin().read_line(&mut s).is_ok() {
        matches!(s.trim().to_lowercase().as_str(), "y" | "yes")
    } else {
        false
    }
}

fn prompt_string(prompt: &str) -> Option<String> {
    use std::io::{self, Write};
    print!("{} ", prompt);
    let _ = io::stdout().flush();
    let mut s = String::new();
    if io::stdin().read_line(&mut s).is_ok() {
        let v = s.trim().to_string();
        if v.is_empty() {
            None
        } else {
            Some(v)
        }
    } else {
        None
    }
}

fn bootstrap_repo_if_missing(path: &std::path::Path) -> anyhow::Result<()> {
    match git::Repository::open(path) {
        Ok(_) => return Ok(()),
        Err(e) => {
            eprintln!("No Git repository found at '{}': {}", path.display(), e);

            if !prompt_yes_no("Initialize a new git repository here? [y/N]") {
                anyhow::bail!("A Git repository is required. Aborting.");
            }
            let mut repo = git::Repository::init_repo(path)?;

            if prompt_yes_no("Create an initial commit adding all files? [Y/n]") {
                repo.initial_commit_all("Initial commit")?;
            }

            println!("Remote setup options:");
            println!("  [e] Add existing remote URL");
            println!("  [g] Create a new remote with GitHub CLI (gh)");
            println!("  [s] Skip");
            if let Some(choice) = prompt_string("Choose (e/g/s):") {
                match choice.as_str() {
                    "e" | "E" => {
                        if let Some(url) = prompt_string(
                            "Enter remote URL (e.g., https://github.com/user/repo.git):",
                        ) {
                            repo.add_remote("origin", &url)?;
                            println!("Remote 'origin' added.");
                        }
                    }
                    "g" | "G" => {
                        let name = prompt_string("Enter new repository name (default: current folder):");
                        let private = prompt_yes_no("Private repo? [y/N]");
                        repo.try_create_remote_with_gh("origin", name.as_deref(), private)?;
                        println!("Created remote with gh and set as origin.");
                    }
                    _ => {
                        println!("Skipping remote setup.");
                    }
                }
            }
            Ok(())
        }
    }
}


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
    execute!(io::stdout(), Clear(ClearType::All))?;
    print!(r"
                
                $$\   $$\           $$\                       $$\                    
                \__|  $$ |          $$ |                      $$ |                   
        $$$$$$\  $$\ $$$$$$\         $$$$$$$\  $$\   $$\  $$$$$$$ | $$$$$$\  $$$$$$\  
        $$  __$$\ $$ |\_$$  _|$$$$$$\ $$  __$$\ $$ |  $$ |$$  __$$ |$$  __$$\ \____$$\ 
        $$ /  $$ |$$ |  $$ |  \______|$$ |  $$ |$$ |  $$ |$$ /  $$ |$$ |  \__|$$$$$$$ |
        $$ |  $$ |$$ |  $$ |$$\       $$ |  $$ |$$ |  $$ |$$ |  $$ |$$ |     $$  __$$ |
        \$$$$$$$ |$$ |  \$$$$  |      $$ |  $$ |\$$$$$$$ |\$$$$$$$ |$$ |     \$$$$$$$ |
        \____$$ |\__|   \____/       \__|  \__| \____$$ | \_______|\__|      \_______|
        $$\   $$ |                              $$\   $$ |                             
        \$$$$$$  |                              \$$$$$$  |                             
        \______/                                \______/                              

            ");

    thread::sleep(Duration::from_millis(2000)); 
    execute!(io::stdout(), Clear(ClearType::All))?;
    
    let cli = Cli::parse();

    let repo_path = cli.repo.unwrap_or_else(|| PathBuf::from("."));

    match cli.command {
        Some(Commands::Ui) | None => {
            bootstrap_repo_if_missing(&repo_path)?;
            let mut app = App::new(repo_path)?;
            app.run().await?;
        }
        Some(Commands::Status) => {
            let repo = git::Repository::open(&repo_path)?;
            let status = repo.status()?;
            println!("{}", status);
        }
        Some(Commands::Log { count}) => {
            let repo = git ::Repository::open(&repo_path)?;
            let commits = repo.get_commits(count)?;
            for commit in commits {
                println!("{:?}", commit);
            }
        }
    }

    Ok(())
}