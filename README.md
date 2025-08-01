# git-hydra
git-hydra is a simple TUI for Git. Build with Rust and ratatui.

### views
- status: view staged, unstaged and untracked files with a diff preview on the side
- commit history: browse commit logs with commit information
- branch management: view, create and checkout between local and remote branches
- remote operations (wip): push to and pull from remote repoisotries
- staging / commit / stashing

### installation
to install git-hydra, simply run the following if you have `cargo` installed:
```cargo install --git https://github.com/asian-mario/git-hydra```

or optionally, you can install and extract via the .tar.gz
```
wget https://github.com/asian-mario/git-hydra/releases/download/[VERSION]/git-hydra-linux-x86_64.tar.gz
tar -xzf git-hydra-linux-x86_64.tar.gz
sudo mv b-top /usr/local/bin/
```