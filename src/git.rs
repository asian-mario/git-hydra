use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use git2::{Repository as Git2Repository, DiffOptions, StatusOptions, PushOptions, RemoteCallbacks, Cred, Progress};
use std::io::{self, Write};
use std::fmt;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Commit {
    pub id: String,
    pub message: String,
    pub author: String,
    pub timestamp: DateTime<Local>,
    pub parents: Vec<String>,
}

impl fmt::Display for Commit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result{
        write!(
            f,
            "{} {} ({})",
            &self.id[..8],
            self.message.lines().next().unwrap_or(""),
            self.author
        )
    }
}

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
                });
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

        let (ahead, behind) = self.calculate_ahead_behind()?;

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
            let author_name = author.name().unwrap_or("unknown").to_string();

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

    fn calculate_ahead_behind(&self) -> Result<(usize, usize)> {
        let head = match self.repo.head(){
            Ok(head) => head,
            Err(_) => return Ok((0, 0)), // empty repo / head not found
        };
        
        let local_oid = head.target().context("failed to get HEAD target.")?;

        let branch_name = head.shorthand().unwrap_or("HEAD");
        let upstream_name = format!("refs/remotes/origin/{}", branch_name);

        let upstream_ref = match self.repo.find_reference(&upstream_name){
            Ok(r) => r,
            Err(_) => return Ok((0, 0))
        };

        let upstream_oid = upstream_ref.target().context("failed to get upstream target.")?;

        let (ahead, behind) = self.repo.graph_ahead_behind(local_oid, upstream_oid)?;

        Ok((ahead, behind))
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

    pub fn stage_file(&self, path: &str) -> Result<()> {
        let mut index = self.repo.index()?;
        index.add_path(Path::new(path))?;
        index.write()?;
        Ok(())
    }

    pub fn unstage_file(&self, path: &str) -> Result<()> {
        let mut index = self.repo.index()?;
        let head = self.repo.head()?.peel_to_tree()?;

        let entry = head.get_path(Path::new(path))?;
        let mut index_entry = git2::IndexEntry{
            ctime: git2::IndexTime::new(0, 0),
            mtime: git2::IndexTime::new(0, 0),
            dev: 0,
            ino: 0,
            mode: entry.filemode() as u32,
            uid: 0,
            gid: 0,
            file_size: entry.to_object(&self.repo)?.as_blob().unwrap().size() as u32,
            id: entry.id(),
            flags: 0,
            flags_extended: 0,
            path: path.as_bytes().to_vec(),
        };

        index.add(&index_entry)?;
        index.write()?;
        Ok(())
    }

    pub fn commit(&self, message: &str) -> Result<git2::Oid>{
        let signature = self.repo.signature()?;
        let mut index = self.repo.index()?;
        let tree_id = index.write_tree()?;
        let tree = self.repo.find_tree(tree_id)?;

        let parent_commit = match self.repo.head(){
            Ok(head) => Some(head.peel_to_commit()?),
            Err(_) => None,
        };

        let parents = if let Some(ref parent) = parent_commit {
            vec![parent]
        } else {
            vec![]
        };

        let commit_id = self.repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parents,
        )?;

        Ok(commit_id)
    }

    pub fn create_branch(&self, name: &str) -> Result<()> {
        let head = self.repo.head()?;
        let commit = head.peel_to_commit()?;
        self.repo.branch(name, &commit, false)?;

        Ok(())
    }

    pub fn checkout_branch(&self, name: &str) -> Result<()>{
        let ref_name = format!("refs/heads/{}", name);
        let obj = self.repo.revparse_single(&ref_name)?;

        self.repo.checkout_tree(&obj, None)?;
        self.repo.set_head(&ref_name)?;
        Ok(())
    }

    pub fn get_file_diff(&self, file_path: &str) -> Result<String> {
        let mut diff_opts = DiffOptions::new();
        diff_opts.pathspec(file_path);

        let diff = self.repo.diff_index_to_workdir(None, Some(&mut diff_opts))?;

        let mut diff_text = String::new();
        diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            match line.origin(){
                '+' => diff_text.push_str(&format!("+{}", std::str::from_utf8(line.content()).unwrap_or(""))),
                '-' => diff_text.push_str(&format!("-{}", std::str::from_utf8(line.content()).unwrap_or(""))),
                ' ' => diff_text.push_str(&format!(" {}", std::str::from_utf8(line.content()).unwrap_or(""))),
                '=' => diff_text.push_str(&format!("={}", std::str::from_utf8(line.content()).unwrap_or(""))),
                '>' => diff_text.push_str(&format!(">{}", std::str::from_utf8(line.content()).unwrap_or(""))),
                '<' => diff_text.push_str(&format!("<{}", std::str::from_utf8(line.content()).unwrap_or(""))),
                'F' => diff_text.push_str(&format!("F{}", std::str::from_utf8(line.content()).unwrap_or(""))),
                'H' => diff_text.push_str(&format!("H{}", std::str::from_utf8(line.content()).unwrap_or(""))),
                _ => diff_text.push_str(std::str::from_utf8(line.content()).unwrap_or("")),
            }
            true
        })?;

        if diff_text.is_empty(){
            let head_tree = self.repo.head()?.peel_to_tree()?;
            let diff = self.repo.diff_tree_to_index(Some(&head_tree), None, Some(&mut diff_opts))?;

            diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
                match line.origin(){
                    '+' => diff_text.push_str(&format!("+{}", std::str::from_utf8(line.content()).unwrap_or(""))),
                    '-' => diff_text.push_str(&format!("-{}", std::str::from_utf8(line.content()).unwrap_or(""))),
                    ' ' => diff_text.push_str(&format!(" {}", std::str::from_utf8(line.content()).unwrap_or(""))),
                    _ => diff_text.push_str(std::str::from_utf8(line.content()).unwrap_or("")),
                }
                true
            })?;
        }

        Ok(diff_text)
    }

    pub fn stash_save(&mut self, message: Option<&str>) -> Result<git2::Oid> {
        let signature = self.repo.signature()?;
        let default_msg = "WIP on branch";
        let stash_msg = message.unwrap_or(default_msg);
        
        let stash_id = self.repo.stash_save(&signature, stash_msg, Some(git2::StashFlags::DEFAULT))?;

        Ok(stash_id)
    }

    pub fn stash_pop(&mut self) -> Result<()>{
        let mut checkout_opts = git2::build::CheckoutBuilder::new();
        checkout_opts.allow_conflicts(true);
        checkout_opts.conflict_style_merge(true);

        let mut stash_apply_opts = git2::StashApplyOptions::new();
        stash_apply_opts.checkout_options(checkout_opts);

        self.repo.stash_pop(0, Some(&mut stash_apply_opts))?;
        Ok(())
    }

    pub fn stash_list(&mut self) -> Result<Vec<String>> {
        let mut stashes = Vec::new();

        self.repo.stash_foreach(|index, message, _oid| {
            stashes.push(format!("stash@{{{}}}: {}", index, message));
            true
        })?;

        Ok(stashes)
    }

    pub fn stash_drop(&mut self, index: usize) -> Result<()> {
        self.repo.stash_drop(index)?;
        Ok(())
    }

    // gaahhhhh
    pub fn push_to_remote(&mut self, remote_name: &str, branch_name: &str) -> Result<()> {
        let mut remote = self.repo.find_remote(remote_name)?;

        let mut callbacks = RemoteCallbacks::new();

        callbacks.credentials(|_url, username_from_url, _allowed_types| {
            if let Ok(cred) = Cred::ssh_key_from_agent(username_from_url.unwrap_or("git")){
                return Ok(cred);
            }

            if let Ok(cred) = Cred::ssh_key(
                username_from_url.unwrap_or("git"),
                None,
                std::path::Path::new(&format!("{}/.ssh/id_rsa", std::env::var("HOME").unwrap_or_default())),
                None,
            ) {
                return Ok(cred)
            }

            Cred::userpass_plaintext("", "")
        });

        callbacks.push_transfer_progress(|current, total, bytes| {
            print!("\rpushing... {}/{} objects ({} bytes)", current, total, bytes);
            io::stdout().flush().unwrap();
        });

        
        let mut push_options = PushOptions::new();
        push_options.remote_callbacks(callbacks);

        let refspec = format!("refs/heads/{}:refs/heads/{}", branch_name, branch_name);
        remote.push(&[&refspec], Some(&mut push_options))?;

        println!("\npush completed successfully!");
        Ok(())
    }

    pub fn pull_from_remote(&mut self, remote_name: &str, branch_name: &str) -> Result<()> {
        let mut remote = self.repo.find_remote(remote_name)?;

        let mut callbacks = RemoteCallbacks::new();

        callbacks.credentials(|_url, username_from_url, _allowed_types| {
            if let Ok(cred) = Cred::ssh_key_from_agent(username_from_url.unwrap_or("git")){
                return Ok(cred);
            }

            if let Ok(cred) = Cred::ssh_key(
                username_from_url.unwrap_or("git"),
                None,
                std::path::Path::new(&format!("{}/.ssh/id_rsa", std::env::var("HOME").unwrap_or_default())),
                None,
            ) {
                return Ok(cred)
            }

            Cred::userpass_plaintext("", "")
        });

        callbacks.transfer_progress(|stats| {
            if stats.received_objects() == stats.total_objects(){
                print!("resolving deltas {}/{} \r", stats.indexed_deltas(), stats.total_deltas());
            } else if stats.total_objects() > 0 {
                (100 * stats.received_objects() / stats.total_objects(),
                stats.received_objects(),
                stats.total_objects(),
                stats.received_bytes());
            }
            io::stdout().flush().unwrap();
            true
        });

        let mut fetch_options = git2::FetchOptions::new();
        fetch_options.remote_callbacks(callbacks);

        let refspec = format!("+refs/heads/{}:refs/remotes/{}/{}", branch_name, remote_name, branch_name);
        remote.fetch(&[&refspec], Some(&mut fetch_options), None)?;

        let remote_branch_name = format!("refs/remotes/{}/{}", remote_name, branch_name);
        let remote_ref = self.repo.find_reference(&remote_branch_name)?;
        let remote_oid = remote_ref.target().context("failed to get remote OID")?;
        // let remote_commit = self.repo.find_commit(remote_oid)?;
        let remote_annotated = self.repo.find_annotated_commit(remote_oid)?;
        let annotated_commits = vec![&remote_annotated];
        
        let head = self.repo.head()?;
        // let local_oid = head.target().context("failed to get local OID")?;
        
        let analysis = self.repo.merge_analysis(&annotated_commits)?;
        
        if analysis.0.is_fast_forward() {
            let mut reference = self.repo.find_reference("HEAD")?;
            reference.set_target(remote_oid, "Fast-forward")?;
            self.repo.set_head("HEAD")?;
            self.repo.checkout_head(Some(
                git2::build::CheckoutBuilder::default()
                    .allow_conflicts(true)
                    .conflict_style_merge(true)
                    .force()
            ))?;
            println!("\nfast-forward merge completed!");
        } else if analysis.0.is_normal() {
            // Need to do a real merge
            println!("\nmerge required - complex operation in WIP");
            return Err(anyhow::anyhow!("manual merge required / please use git command line"));
        } else if analysis.0.is_up_to_date() {
            println!("\nalready up to date!");
        } else {
            println!("\nno merge possible");
            return Err(anyhow::anyhow!("cannot merge - conflicting changes"));
        }
        
        Ok(())
    }

    pub fn get_remotes(&self) -> Result<Vec<String>>{
        let remotes = self.repo.remotes()?;
        Ok(remotes.iter()
            .filter_map(|r| r.map(|s| s.to_string()))
            .collect())
    }

    pub fn get_remote_url(&self, remote_name: &str) -> Result<Option<String>>{
        let remote = self.repo.find_remote(remote_name)?;
        Ok(remote.url().map(|s| s.to_string()))
    }

    pub fn get_current_branch(&self) -> Result<String>{
        let head = self.repo.head()?;
        if let Some(name) = head.shorthand(){
            Ok(name.to_string())
        } else {
            Ok("HEAD".to_string())
        }
    }
}