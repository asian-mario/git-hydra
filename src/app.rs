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

use std::collections::HashMap;
use crate::git::*;
use crate::ui;

#[derive(Debug, PartialEq)]
pub enum AppMode {
    Status,
    Log,
    Branches,
    CommitDialog,
    CreateBranchDialog,
    StashDialog,
    StashList,
    RemoteOperations,
    MergeConflict,
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

    pub diff_scroll: u16,

    pub branch_name: String,
    pub stash_message: String,
    pub stashes: Vec<String>,
    pub selected_stash: usize,

    // remote stuff
    pub remotes: Vec<String>,
    pub selected_remote: usize,
    pub current_branch: String,
    pub is_pushing: bool,
    pub is_pulling: bool,
    pub push_progress: Option<String>,
    pub pull_progress: Option<String>,

    pub merge_conflict: Option<MergeConflict>,
    pub selected_conflict_file: usize,
    pub selected_conflict_hunk: usize,
    pub conflict_resolutions: std::collections::HashMap<(usize, usize), MergeResolution>,
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

            diff_scroll: 0,

            branch_name: String::new(),
            stash_message: String::new(),
            stashes: Vec::new(),
            selected_stash: 0,

            remotes: Vec::new(),
            selected_remote: 0,
            current_branch: String::new(),
            is_pushing: false,
            is_pulling: false,
            push_progress: None,
            pull_progress: None,

            merge_conflict: None,
            selected_conflict_file: 0,
            selected_conflict_hunk: 0,
            conflict_resolutions: std::collections::HashMap::new(),
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

    pub fn get_selected_file_path(&self) -> Option<String> {
        if let Some(status) = &self.status {
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

    fn push_current_branch(&mut self) -> Result<()> {
        if self.selected_remote < self.remotes.len() {
            let remote_name = &self.remotes[self.selected_remote];
            self.is_pushing = true;
            self.push_progress = Some("pushing to remote...".to_string());

            match self.repo.push_to_remote(remote_name, &self.current_branch) {
                Ok(_) => {
                        self.push_progress = Some("Push completed successfully!".to_string());
                    self.refresh_data()?;
                }
                Err(e) => {
                    self.error_message = Some(format!("push failed: {}", e));
                    self.push_progress = None;
                }
            }
            self.is_pushing = false;
        }
        Ok(())
    }

    fn pull_current_branch(&mut self) -> Result<()> {
        if self.selected_remote < self.remotes.len() {
            let remote_name = &self.remotes[self.selected_remote];
            self.is_pulling = true;
            self.pull_progress = Some("pulling from remote...".to_string());

            match self.repo.pull_from_remote(remote_name, &self.current_branch) {
                Ok(_) => {
                    self.pull_progress = Some("pull completed successfully!".to_string());
                    self.refresh_data()?;
                    if self.mode == AppMode::MergeConflict {
                        self.error_message = Some("merge conflict detected after pull, please resolve.".to_string());
                    }
                }
                Err(e) => {
                    self.pull_progress = None;
                    let error_mesg = e.to_string();
                    if error_mesg.contains("merge conflicts require resolution") {
                        self.refresh_data()?;
                        if self.mode == AppMode::MergeConflict {
                            self.error_message = Some("merge conflict detected after pull, please resolve.".to_string());
                        } else {
                            self.error_message = Some(format!("pull failed: {}", e));
                        }
                    } else {
                        self.error_message = Some(format!("pull failed: {}", e));
                    }
                }
            }
            self.is_pulling = false;
        }
        Ok(())
    }

    fn handle_key_event(&mut self, key: KeyCode) -> Result<()> {
        // clear error message on any key press -> visual oops
        self.error_message = None;
    
        self.push_progress = None;
        self.pull_progress = None;

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
            AppMode::CreateBranchDialog => {
                match key {
                    KeyCode::Esc => {
                        self.mode = AppMode::Branches;
                        self.branch_name.clear()
                    }
                    KeyCode::Enter => {
                        if !self.branch_name.trim().is_empty() {
                            match self.repo.create_branch(&self.branch_name) {
                                Ok(_) => {
                                    self.mode = AppMode::Branches;
                                    self.branch_name.clear();
                                    self.refresh_data()?;
                                }
                                Err(e) => {
                                    self.error_message = Some(format!("failed to create branch: {}", e));
                                }
                            }
                        }
                    }
                    KeyCode::Char(c) => {
                        self.branch_name.push(c);
                    }
                    KeyCode::Backspace => {
                        self.branch_name.pop();
                    }

                    _ => {}
                }
                return Ok(());
            }
            AppMode::StashDialog => {
                match key {
                    KeyCode::Esc => {
                        self.mode = AppMode::Status;
                        self.stash_message.clear();
                    }
                    KeyCode::Enter => {
                        let msg = if self.stash_message.trim().is_empty(){
                            None
                        } else {
                            Some(self.stash_message.as_str())
                        };

                        match self.repo.stash_save(msg){
                            Ok(_) => {
                                self.mode = AppMode::Status;
                                self.stash_message.clear();
                                self.refresh_data()?;
                            }
                            Err(e) => {
                                self.error_message = Some(format!("failed to stash: {}", e));
                            }
                        }
                    }
                    KeyCode::Char(c) => {
                        self.stash_message.push(c);
                    }
                    KeyCode::Backspace => {
                        self.stash_message.pop();
                    }
                    _ => {}
                }
                return Ok(());
            }
            AppMode::StashList => {
                match key {
                    //idiot forgot to add key handling
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
                    KeyCode::Esc => {
                        self.mode = AppMode::Status;
                        return Ok(());
                    }
                    KeyCode::Up => {
                        if self.selected_stash > 0 {
                            self.selected_stash -= 1;
                        }
                        return Ok(());
                    }
                    KeyCode::Down => {
                        if self.selected_stash + 1 < self.stashes.len() {
                            self.selected_stash += 1;
                        }
                        return Ok(());
                    }
                    KeyCode::Enter => {
                        match self.repo.stash_pop() {
                            Ok(_) => {
                                self.mode = AppMode::Status;
                                self.refresh_data()?;
                            }
                            Err(e) => {
                                self.error_message = Some(format!("failed to pop stash: {}", e));
                            }
                        }
                        return Ok(());
                    }
                    KeyCode::Delete => {
                        if self.selected_stash < self.stashes.len() {
                            match self.repo.stash_drop(self.selected_stash) {
                                Ok(_) => {
                                    self.refresh_data()?;
                                    if self.selected_stash >= self.stashes.len() && !self.stashes.is_empty() {
                                        self.selected_stash = self.stashes.len() -1;
                                    }
                                }
                                Err(e) => {
                                    self.error_message = Some(format!("failed to drop stash: {}", e));
                                }
                            }
                        }
                        return Ok(());
                    }
                    _ => {}
                }
                return Ok(());
            }
            AppMode::RemoteOperations => {
                match key {
                    KeyCode::Up => {
                        if self.selected_remote > 0{
                            self.selected_remote -= 1;
                        }
                        return Ok(());
                    }
                    KeyCode::Down => {
                        if self.selected_remote + 1 < self.remotes.len() {
                            self.selected_remote += 1;
                        }
                        return Ok(())
                    }
                    _ => {}
                }
            }
            AppMode::MergeConflict => {
                match key {
                    KeyCode::Up => {
                        if self.selected_conflict_hunk > 0 {
                            self.selected_conflict_hunk -= 1;
                        } else if self.selected_conflict_file > 0 {
                            self.selected_conflict_file -= 1;
                            if let Some(file) = self.get_current_conflict_file() {
                                self.selected_conflict_hunk = file.conflicts.len().saturating_sub(1);
                            }
                        }
                        return Ok(());
                    }
                    KeyCode::Down => {
                        if let Some(file) = self.get_current_conflict_file() {
                            if self.selected_conflict_hunk + 1 < file.conflicts.len() {
                                self.selected_conflict_hunk += 1;
                            } else if let Some(merge_conflict) = &self.merge_conflict {
                                if self.selected_conflict_file + 1 < merge_conflict.files.len() {
                                    self.selected_conflict_file += 1;
                                    self.selected_conflict_hunk = 0;
                                }

                            }
                        }
                        return Ok(());
                    }
                    KeyCode::Right => {
                        if let Some(merge_conflict) = &self.merge_conflict {
                            if self.selected_conflict_file + 1 < merge_conflict.files.len() {
                                self.selected_conflict_file += 1;
                                self.selected_conflict_hunk = 0;
                            }
                        }
                        return Ok(());
                    }

                    // resolution choices
                    KeyCode::Char('o') => {
                        self.set_current_resolution(MergeResolution::KeepOurs);
                        return Ok(());
                    }
                    KeyCode::Char('t') => {
                        self.set_current_resolution(MergeResolution::KeepTheirs);
                        return Ok(());
                    }
                    KeyCode::Char('b') => {
                        self.set_current_resolution(MergeResolution::KeepBoth);
                        return Ok(());
                    }
                    KeyCode::Char('e') => {
                        // TODO: edit custom res.
                        // self.set_current_resolution(MergeResolution::Edit);
                        return Ok(());
                    }

                    
                    KeyCode::Char('c') => {
                        if self.can_complete_merge() {
                            match self.complete_merge("Merge commit") {
                                Ok(_) =>{
                                    self.refresh_data()?;
                                }
                                Err(e) => {
                                    self.error_message = Some(format!("failed to complete merge: {}", e));
                                }
                            }
                        } else {
                            self.error_message = Some("cannot complete merge, some conflicts are unresolved.".to_string());
                        }
                        return Ok(());
                    }

                    KeyCode::Char('a') => {
                        match self.abort_merge() {
                            Ok(_) => {
                                self.refresh_data()?;
                            }
                            Err(e) => {
                                self.error_message = Some(format!("failed to abort merge: {}", e));
                            }
                        }
                    }

                    KeyCode::F(5) => {
                        self.refresh_data()?;
                        return Ok(());
                    }

                    KeyCode::Esc => {
                        if self.merge_conflict.is_some() {
                            self.mode = AppMode::Status;
                        } else {
                            self.error_message = Some("cannot exit merge mode while conflicts exist. use 'a' to abort or 'c' to complete.".to_string());
                        }
                        return Ok(());
                    }

                    _ => {}
                }
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
            KeyCode::Char('4') => {
                self.mode = AppMode::StashList;
                self.refresh_data()?;
            }
            KeyCode::Char('5') => {
                self.mode = AppMode::RemoteOperations;
                self.refresh_data()?;
            }
            KeyCode::Char('p') => {
                if self.mode == AppMode::RemoteOperations && !self.remotes.is_empty() {
                    self.push_current_branch()?;
                }
            }
            KeyCode::Char('u') => {
                if self.mode == AppMode::RemoteOperations && !self.remotes.is_empty() {
                    self.pull_current_branch()?;
                }
            }
            KeyCode::Char('s') => {
                self.mode = AppMode::StashDialog;
                self.stash_message.clear();
            }
            KeyCode::Char('n') => {
                if self.mode == AppMode::Branches {
                    self.mode = AppMode::CreateBranchDialog;
                    self.branch_name.clear();
                }
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
            
            KeyCode::Char('m') => {
                if let Ok(Some(_)) = self.repo.detect_merge_conflicts() {
                    self.mode = AppMode::MergeConflict;
                    self.refresh_data()?;
                } else {
                    self.error_message = Some("no merge conflicts detected.".to_string());
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
                            self.diff_scroll = 0;
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
                                self.diff_scroll = 0;

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
                                            self.refresh_data()?;
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
                if let Some(status) = &self.status{
                    let total_files = status.staged.len() + status.unstaged.len() + status.untracked.len();
                    if self.selected_file < total_files {
                        if let Some(file_path) = self.get_selected_file_path() {
                            if self.selected_file < status.staged.len() {
                                if let Err(e) = self.repo.unstage_file(&file_path){
                                    self.error_message = Some(format!("failed to unstage: {}", e));
                                } else {
                                    self.refresh_data()?;
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

            KeyCode::PageUp => {
                if self.mode == AppMode::Status {
                    self.diff_scroll = self.diff_scroll.saturating_sub(10);
                }
            }
            KeyCode::PageDown => {
                if self.mode == AppMode::Status {
                    self.diff_scroll = self.diff_scroll.saturating_add(10);
                }
            }
            KeyCode::Home => {
                if self.mode == AppMode::Status {
                    self.diff_scroll = 0;
                }
            }
            KeyCode::End => {
                if self.mode == AppMode::Status {
                    self.diff_scroll += 1000;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn refresh_data(&mut self) -> Result<()> {
        match self.repo.detect_merge_conflicts(){
            Ok(Some(conflict)) => {
                self.merge_conflict = Some(conflict);
                if self.mode != AppMode::MergeConflict {
                    self.mode = AppMode::MergeConflict;
                    self.selected_conflict_file = 0;
                    self.selected_conflict_hunk = 0;
                    self.conflict_resolutions.clear();
                }
                return Ok(());
            }
            Ok(None) => {
                if self.mode == AppMode::MergeConflict {
                    self.merge_conflict = None;
                    self.conflict_resolutions.clear();
                    self.mode = AppMode::Status;
                }
            }
            Err(e) => {
                self.error_message = Some(format!("failed to detect merge conflicts: {}", e));
            }
        }
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
            AppMode::StashList => {
                self.stashes = self.repo.stash_list()?;
                if self.selected_stash >= self.stashes.len() {
                    self.selected_stash = self.stashes.len().saturating_sub(1);
                }

            }
            AppMode::RemoteOperations => {
                self.remotes = self.repo.get_remotes()?;
                self.current_branch = self.repo.get_current_branch()?;
                if self.selected_remote >= self.remotes.len() {
                    self.selected_remote = self.remotes.len().saturating_sub(1);
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub fn get_current_conflict_file(&self) -> Option<&ConflictedFile> {
        self.merge_conflict.as_ref()
            .and_then(|mc | mc.files.get(self.selected_conflict_file))
    }

    pub fn get_current_conflict_hunk(&self) -> Option<&ConflictHunk> {
        self.get_current_conflict_file()
            .and_then(|file| file.conflicts.get(self.selected_conflict_hunk))
    }

    pub fn get_current_resolution(&self) -> Option<&MergeResolution> {
        self.conflict_resolutions.get(&(self.selected_conflict_file, self.selected_conflict_hunk))
    }

    pub fn set_current_resolution(&mut self, resolution: MergeResolution) {
        self.conflict_resolutions.insert((self.selected_conflict_file, self.selected_conflict_hunk), resolution);
    }

    pub fn can_complete_merge(&self) -> bool {
        if let Some(merge_conflict) = &self.merge_conflict {
            for (file_idx, file) in merge_conflict.files.iter().enumerate() {
                for hunk_idx in 0 .. file.conflicts.len() {
                    if !self.conflict_resolutions.contains_key(&(file_idx, hunk_idx)) {
                        return false; 
                    }
                }
            }
            true
        } else {
            false
        }
    }

    pub fn complete_merge(&mut self, commit_message: &str) -> Result<()> {
        if let Some(merge_conflict) = &self.merge_conflict {
            self.repo.resolve_conflicts(&self.conflict_resolutions, merge_conflict)?;
            self.repo.complete_merge(commit_message)?;

            self.merge_conflict = None;
            self.conflict_resolutions.clear();
            self.mode = AppMode::Status;
            self.refresh_data()?;
        }
        Ok(())
    }

    pub fn abort_merge(&mut self) -> Result<()> {
        self.repo.abort_merge()?;
        self.merge_conflict = None;
        self.conflict_resolutions.clear();
        self.mode = AppMode::Status;
        self.refresh_data()?;
        Ok(())
    }
}
