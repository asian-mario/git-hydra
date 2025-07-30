use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use crate::git::{Repository, RepoStatus, Commit};
use crate::ui;

#[derive(Debug, PartialEq)]
pub enum AppMode {
    Status,
    Log,
    Branches,
    CommitDialog,
}

pub struct App {
    pub should_quit: bool,
    pub mode: AppMode,
    pub repo: Repository,
    pub status: Option<RepoStatus>,
    pub commits: Vec<Commit>,
    pub branches: Vec<String>,
    pub selected_commit: usize,
    pub selected_file: usize,

    pub commit_message: String,
    pub error_message: Option<String>,
}

impl App {
    pub fn new(repo_path: PathBuf) -> Result<Self> {
        let repo = Repository::open(repo_path)?;

        Ok(Self {
            should_quit: false,
            mode: AppMode::Status,
            repo,
            status: None,
            commits: Vec::new(),
            branches: Vec::new(),
            selected_commit: 0,
            selected_file: 0,
            
            commit_message: String::new(),
            error_message: None,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        // setup the terminal and init.
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        self.refresh_data()?;

        let result = self.run_app(&mut terminal).await;

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    async fn run_app<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        loop {
            terminal.draw(|f| ui::draw(f, self))?;

            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key_event(key.code)?;
                }
            }

            if self.should_quit {
                break;
            }
        }
        Ok(())
    }

    fn get_selected_file_path(&self) -> Option<String> {
        if let Some(status) = &self.status{
            let mut all_files = Vec::new();

            // staged / unstaged / untracked

            for file in &status.staged {
                all_files.push(file.path.clone());
            }
            for file in &status.unstaged {
                all_files.push(file.path.clone());
            }
            for file in &status.untracked {
                all_files.push(file.path.clone());
            }

            all_files.get(self.selected_file).cloned()
        } else {
            None
        }
    }

    fn handle_key_event(&mut self, key: KeyCode) -> Result<()> {
        // clear error message on any key press -> visual oops
        self.error_message = None;

        match self.mode {
            AppMode::CommitDialog => {
                match key {
                    KeyCode::Esc => {
                        self.mode = AppMode::Status;
                        self.commit_message.clear();
                    }
                    KeyCode::Enter => {
                        if !self.commit_message.trim().is_empty() {
                            match self.repo.commit(&self.commit_message) {
                                Ok(_) => {
                                    self.mode = AppMode::Status;
                                    self.commit_message.clear();
                                    self.refresh_data()?;
                                }
                                Err(e) => {
                                    self.error_message = Some(format!("commit failed: {}", e));
                                }
                            }
                        }
                    }
                    KeyCode::Char(c) => {
                        self.commit_message.push(c);
                    }
                    KeyCode::Backspace => {
                        self.commit_message.pop();
                    }
                    _ => {}
                }
                return Ok(());
            }
            _ => {}
        }
        match key {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('1') => {
                self.mode = AppMode::Status;
                self.refresh_data()?;
            }
            KeyCode::Char('2') => {
                self.mode = AppMode::Log;
                self.refresh_data()?;
            }
            KeyCode::Char('3') => {
                self.mode = AppMode::Branches;
                self.refresh_data()?;
            }
            KeyCode::Char('c') => {
                // Open dialog if there are staged files
                if let Some(status) = &self.status {
                    if !status.staged.is_empty() {
                        self.mode = AppMode::CommitDialog;
                        self.commit_message.clear();
                    } else {
                        self.error_message = Some("no staged files to commit.".to_string())
                    }
                }
            }
            KeyCode::Up => {
                match self.mode {
                    AppMode::Log => {
                        if self.selected_commit > 0 {
                            self.selected_commit -= 1;
                        }
                    }
                    AppMode::Status => {
                        if self.selected_file > 0 {
                            self.selected_file -= 1;
                        }
                    }
                    AppMode::Branches => {
                        if self.selected_file > 0{
                            self.selected_file -= 1;
                        }
                    }
                    _ => {}
                }
            }
            KeyCode::Down => {
                match self.mode {
                    AppMode::Log => {
                        if self.selected_commit + 1 < self.commits.len() {
                            self.selected_commit += 1;
                        }
                    }
                    AppMode::Status => {
                        if let Some(status) = &self.status {
                            let total_files = status.staged.len() + status.unstaged.len() + status.untracked.len();
                            if self.selected_file + 1 < total_files {
                                self.selected_file += 1;
                            }
                        }
                    }
                    AppMode::Branches => {
                        if self.selected_file + 1 < self.branches.len() {
                            self.selected_file += 1;
                        }
                    }

                    _ => {}
                }
            }
            KeyCode::F(5) => {
                self.refresh_data()?;
            }
            KeyCode::Enter => {
                match self.mode {
                    AppMode::Status => {
                        if let Some(status) = &self.status {
                            let total_files = status.staged.len() + status.unstaged.len() + status.untracked.len();
                            if self.selected_file < total_files {
                                if let Some(file_path) = self.get_selected_file_path() {
                                    if self.selected_file < status.staged.len() {
                                        if let Err(e) = self.repo.unstage_file(&file_path){
                                            self.error_message = Some(format!("failed to unstage: {}", e));
                                        } else {
                                            self.refresh_data();
                                        }
                                    } else {
                                        if let Err(e) = self.repo.stage_file(&file_path){
                                            self.error_message = Some(format!("failed to stage: {}", e));

                                        } else {
                                            self.refresh_data()?;
                                        }
                                    }
                                    self.refresh_data()?;
                                } 
                            }
                        }
                    }
                    AppMode::Branches => {
                        if self.selected_file < self.branches.len() {
                            let branch = &self.branches[self.selected_file];
                            if !branch.starts_with("origin/"){
                                match self.repo.checkout_branch(branch){
                                    Ok(_) => {
                                        self.refresh_data()?;
                                    }
                                    Err(e) => {
                                        self.error_message = Some(format!("failed to checkout: {}", e))
                                    }
                                }
                            }
                        } else {
                            self.error_message = Some("cannot checkout remote branch directly.".to_string());
                        }
                    }
                    _ => {}
                }
            }
            
            // this is staged by git-hydra!
            KeyCode::Char(' ') => {
                if let Some(status) = &self.status {
                    let total_files = status.staged.len() + status.unstaged.len() + status.untracked.len();
                    if self.selected_file < total_files {
                        if let Some(file_path) = self.get_selected_file_path() {
                            if self.selected_file < status.staged.len() {
                                if let Err(e) = self.repo.unstage_file(&file_path){
                                    self.error_message = Some(format!("failed to unstage: {}", e));
                                } else {
                                    self.refresh_data();
                                }
                            } else {
                                if let Err(e) = self.repo.stage_file(&file_path){
                                    self.error_message = Some(format!("failed to stage: {}", e));

                                } else {
                                    self.refresh_data()?;
                                }
                            }
                            self.refresh_data()?;
                        } 
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn refresh_data(&mut self) -> Result<()> {
        match self.mode {
            AppMode::Status => {
                self.status = Some(self.repo.status()?);
            }
            AppMode::Log => {
                self.commits = self.repo.get_commits(50)?;
                if self.selected_commit >= self.commits.len() {
                    self.selected_commit = self.commits.len().saturating_sub(1);
                }
            }
            AppMode::Branches => {
                self.branches = self.repo.get_branches()?;
            }

            _ => {}

        }

        Ok(())
    }
}