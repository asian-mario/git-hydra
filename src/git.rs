use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use git2::{Repository as Git2Repository, StatusOptions};
use std::fmt;
use std::path::Path;

/// i'm convinced theres a shadow man in vscode
/// i did not write a lot of these lines
/// somehow they showed up
/// when did i ever want to use core??
#[derive(Debug, Clone)]
pub struct Commit {
    pub id: String,
    pub message: String,
    pub author: String,
    pub timestamp: DateTime<Local>,
    pub parents: Vec<String>,
}

// i also somehow didn't write a commit struct, how?

#[derive(Debug, Clone)]
pub struct FileStatus {
    pub path: String,
    pub status: String,
}

#[derive(Debug)]
pub struct RepoStatus {
    pub branch: String,
    pub ahead: usize,
    pub behind: usize,
    pub staged: Vec<FileStatus>,
    pub unstaged: Vec<FileStatus>,
    pub untracked: Vec<FileStatus>,
}

impl fmt::Display for RepoStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "branch: {}", self.branch)?;
        if self.ahead > 0 {
            writeln!(f, "ahead by {} commits.", self.ahead)?;
        }
        if self.behind > 0 {
            writeln!(f, "behind by {} commits.", self.behind)?;
        }

        if !self.staged.is_empty(){
            writeln!(f, "\nstaged changes:")?;
            for file in &self.staged {
                writeln!(f, " {} {}", file.status, file.path)?;
            }
        }

        if !self.unstaged.is_empty(){
            writeln!(f, "\nunstaged changes:")?;
            for file in &self.unstaged {
                writeln!(f, " {} {}", file.status, file.path)?;
            }
        }

        if !self.untracked.is_empty() {
            writeln!(f, "\nuntracked files:")?;
            for file in &self.untracked {
                writeln!(f, " {} {}", file.status, file.path)?;
            }
        }

        Ok(())
    }
}

pub struct Repository {
    repo: Git2Repository
}

impl Repository {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let repo = Git2Repository::discover(path)
            .context("failed to find Git repository. try again.")?;
        Ok(Self { repo })
    }

    pub fn status(&self) -> Result<RepoStatus> {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true);
        opts.include_ignored(false);

        let statuses = self.repo.statuses(Some(&mut opts))?;

        let mut staged = Vec::new();
        let mut unstaged = Vec::new();
        let mut untracked = Vec::new();

        for entry in statuses.iter() {
            let path = entry.path().unwrap_or("").to_string();
            let status = entry.status();

            if status.is_index_new() || status.is_index_modified() || status.is_index_deleted() {
                let status_char = if status.is_index_new() { "A" }
                else if status.is_index_modified() { "M" }
                else if status.is_index_deleted() { "D" }
                else { "?" };

                staged.push(FileStatus {
                    path: path.clone(),
                    status: status_char.to_string(),
                })
            }

            if status.is_wt_new() {
                untracked.push(FileStatus {
                    path,
                    status: "??".to_string(),
                });
            } else if status.is_wt_modified() || status.is_wt_deleted() {
                let status_char = if status.is_wt_modified() { "M" } else { "D" };
                unstaged.push(FileStatus {
                    path,
                    status: status_char.to_string(),
                });
            }
        }

        let head = self.repo.head()?;
        let branch = if let Some(name) = head.shorthand() {
            name.to_string()
        } else {
            "HEAD".to_string()
        };

        //TODO: lazy rn so i'll calculate the ahead/behind counts in a bit
        let ahead = 0;
        let behind = 0;

        Ok(RepoStatus {
            branch,
            ahead,
            behind,
            staged,
            unstaged,
            untracked,
        })
    }

    pub fn get_commits(&self, count: usize) -> Result<Vec<Commit>> {
        let mut revwalk = self.repo.revwalk()?;
        revwalk.push_head()?;
        revwalk.set_sorting(git2::Sort::TIME)?;

        let mut commits = Vec::new();

        for (i, oid) in revwalk.enumerate() {
            if i >= count { break; }

            let oid = oid?;
            let commit = self.repo.find_commit(oid)?;

            let message = commit.message().unwrap_or("").to_string();
            let author = commit.author();
            let author_name = author.name().unwrap_or("Unknown").to_string();

            let timestamp = DateTime::from_timestamp(commit.time().seconds(), 0)
                .unwrap_or_default()
                .with_timezone(&Local);

            let parents = commit.parents()
                .map(|p| p.id().to_string())
                .collect();
            
            commits.push(Commit {
                id: oid.to_string(),
                message,
                author: author_name,
                timestamp,
                parents,
            });

        }
        Ok(commits)
    }

    pub fn get_branches(&self) -> Result<Vec<String>> {
        let branches = self.repo.branches(None)?;
        let mut branch_names = Vec::new();

        for branch in branches {
            let (branch, _) = branch?;
            if let Some(name) = branch.name()? {
                branch_names.push(name.to_string());
            }
        }

        Ok(branch_names)
    }
}