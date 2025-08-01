use ratatui::{
    backend::Backend,
    layout::{self, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Clear, List, ListItem, ListState, Paragraph, Tabs, Wrap
    },
    Frame,
};

use crate::app::{App, AppMode};


pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(3)])
        .split(f.area());

    draw_header(f, chunks[0], app);
    
    match app.mode {
        AppMode::Status => draw_status_view(f, chunks[1], app),
        AppMode::Log => draw_log_view(f, chunks[1], app),
        AppMode::Branches => draw_branches_view(f, chunks[1], app),
        AppMode::StashList => draw_stash_view(f, chunks[1], app),

        AppMode::CommitDialog => {
            draw_status_view(f, chunks[1], app);
            draw_commit_dialog(f, f.area(), app);
        }
        AppMode::CreateBranchDialog => {
            draw_branches_view(f, chunks[1], app);
            draw_create_branch_dialog(f, f.area(), app);
        }
        AppMode::StashDialog => {
            draw_status_view(f, chunks[1], app);
            draw_stash_dialog(f, f.size(), app);
        }
        AppMode::RemoteOperations => draw_remote_view(f, chunks[1], app),
    }

    if let Some(error) = &app.error_message {
        draw_error_popup(f, f.area(), error);
    }
    draw_footer(f, chunks[2]);
}

fn draw_header(f: &mut Frame, area: Rect, app: &App){
    let titles = vec!["status (1)", "log (2)", "branches (3)", "stashes (4)", "remote (5)"];
    let selected = match app.mode {
        AppMode::Status | AppMode::CommitDialog | AppMode::StashDialog => 0,
        AppMode::Log => 1,
        AppMode::Branches | AppMode::CreateBranchDialog => 2,
        AppMode::StashList => 3,
        AppMode::RemoteOperations => 4,
    };

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title("git-hydra"))
        .select(selected)
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    
    f.render_widget(tabs, area);
}

fn draw_footer(f: &mut Frame, area: Rect) {
    let help_text = match f.area().width > 120 {
        true => "↑/↓: navigate | 1/2/3/4: switch tabs | enter: action | s: stash | n: new branch | c: commit | pgup/down: scroll diff view | F5: refresh | q: quit",
        false => "↑/↓: nav | 1/2/3/4: tabs | enter: action | s: stash | n: branch | c: commit | pgup/down: scroll diff | q: quit",
    };

    let help = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::Gray))
        .wrap(Wrap { trim: true });

    f.render_widget(help, area);
}

fn draw_status_view(f: &mut Frame, area: Rect, app: &App) {
    if let Some(status) = &app.status{
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(30), Constraint::Percentage(30)])
            .split(area);

        draw_file_changes(f, chunks[0], app, status);

        draw_repo_info(f, chunks[1], status);

        draw_file_diff(f, chunks[2], app);
    } else {
        let loading = Paragraph::new("loading repository status...")
            .block(Block::default().borders(Borders::ALL).title("status."))
            .style(Style::default().fg(Color::Cyan));

        f.render_widget(loading, area);
    }
}


fn draw_file_changes(f: &mut Frame, area: Rect, app: &App, status: &crate::git::RepoStatus){
    let mut items = Vec::new();
    let mut current_index = 0;

    if !status.staged.is_empty(){
        items.push(ListItem::new(Line::from(vec![
            Span::styled("── staged changes ──", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
        ])));

        for file in &status.staged {
            let style = if current_index == app.selected_file {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default().fg(Color::Green)
            };

            items.push(ListItem::new(Line::from(vec![
                Span::styled(format!(" {} ", file.status), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::styled(&file.path, style),
            ])));
            current_index += 1;
        }
    }

    if !status.unstaged.is_empty(){
        items.push(ListItem::new(Line::from(vec![
            Span::styled("── unstaged changes ──", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        ])));

        for file in &status.unstaged {
            let style = if current_index == app.selected_file {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default().fg(Color::Red)
            };

            items.push(ListItem::new(Line::from(vec![
                Span::styled(format!(" {} ", file.status), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::styled(&file.path, style),
            ])));
            current_index += 1;
        }
    }

    if !status.untracked.is_empty(){
        items.push(ListItem::new(Line::from(vec![
            Span::styled("── untracked files ──", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        ])));

        for file in &status.untracked {
            let style = if current_index == app.selected_file {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default().fg(Color::Cyan)
            };

            items.push(ListItem::new(Line::from(vec![
                Span::styled(" ?? ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(&file.path, style),
            ])));
            current_index += 1;
        }
    }

    if items.is_empty(){
        items.push(ListItem::new(Line::from(vec![
            Span::styled("✔ working tree clean.", Style::default().fg(Color::Green))
        ])));
    }

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("changes"))
        .style(Style::default().fg(Color::White));

    f.render_widget(list, area);
}

fn draw_repo_info(f: &mut Frame, area: Rect, status: &crate::git::RepoStatus){
    let mut text = Vec::new();

    text.push(Line::from(vec![
        Span::styled("branch: ", Style::default().fg(Color::Gray)),
        Span::styled(&status.branch, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    ]));
    
    if status.ahead > 0 {
        text.push(Line::from(vec![
            Span::styled("↑ ahead: ", Style::default().fg(Color::Green)),
            Span::styled(status.ahead.to_string(), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(" commits", Style::default().fg(Color::Gray))
        ]));
    }

    if status.behind > 0 {
        text.push(Line::from(vec![
            Span::styled("↓ behind: ", Style::default().fg(Color::Red)),
            Span::styled(status.behind.to_string(), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::styled(" commits", Style::default().fg(Color::Gray)),
        ]));
    }
    
    if status.ahead == 0 && status.behind == 0{
        text.push(Line::from(vec![
            Span::styled("✓ up to date.", Style::default().fg(Color::Green)),
        ]));
    }
    
    text.push(Line::from(""));

    let total_changes = status.staged.len() + status.unstaged.len() + status.untracked.len();
    if total_changes > 0{
        text.push(Line::from(vec![
            Span::styled("files changed: ", Style::default().fg(Color::Gray)),
            Span::styled(total_changes.to_string(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]));
        
        if !status.staged.is_empty() {
            text.push(Line::from(vec![
                Span::styled("  staged: ", Style::default().fg(Color::Green)),
                Span::styled(status.staged.len().to_string(), Style::default().fg(Color::Green)),
            ]));
        }
        
        if !status.unstaged.is_empty() {
            text.push(Line::from(vec![
                Span::styled("  unstaged: ", Style::default().fg(Color::Red)),
                Span::styled(status.unstaged.len().to_string(), Style::default().fg(Color::Red)),
            ]));
        }
        
        if !status.untracked.is_empty() {
            text.push(Line::from(vec![
                Span::styled("  untracked: ", Style::default().fg(Color::Cyan)),
                Span::styled(status.untracked.len().to_string(), Style::default().fg(Color::Cyan)),
            ]));
        }
    }

    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("repository info."))
        .wrap(Wrap { trim: true });
    
    f.render_widget(paragraph, area);

}

fn draw_file_diff(f: &mut Frame, area: Rect, app: &App){
    if let Some(file_path) = app.get_selected_file_path() {
        match app.repo.get_file_diff(&file_path){
            Ok(diff_text) => {
                if diff_text.is_empty(){
                    let no_diff = Paragraph::new("no changes to display.")
                        .block(Block::default().borders(Borders::ALL).title("diff."))
                        .style(Style::default().fg(Color::Gray));

                    f.render_widget(no_diff, area);
                } else {
                    let lines: Vec<Line> = diff_text
                        .lines()
                        .map(|line| {
                            if line.starts_with('+') && !line.starts_with("+++") {
                                Line::from(Span::styled(line, Style::default().fg(Color::Green)))
                            } else if line.starts_with('-') && !line.starts_with("---") {
                                Line::from(Span::styled(line, Style::default().fg(Color::Red)))
                            } else if line.starts_with("@@") {
                                Line::from(Span::styled(line, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)))
                            } else if line.starts_with("+++") || line.starts_with("---") {
                                Line::from(Span::styled(line, Style::default().fg(Color::Yellow)))
                            } else {
                                Line::from(Span::styled(line, Style::default().fg(Color::White)))
                            }
                        })
                        .collect();
                        
                    let content_height = lines.len() as u16;
                    let viewport_height = area.height.saturating_sub(2);
                    let max_scroll = content_height.saturating_sub(viewport_height);

                    let actual_scroll = app.diff_scroll.min(max_scroll);

                    let title = if content_height > viewport_height {
                        format!("diff: {} ({}%)", file_path,
                            (actual_scroll * 100 / max_scroll.max(1)))
                    } else {
                        format!("diff: {}", file_path)
                    };
                        
                    let diff_paragraph = Paragraph::new(lines)
                        .block(Block::default().borders(Borders::ALL).title(title))
                        .wrap(Wrap { trim: false })
                        .scroll((actual_scroll, 0));
                    
                    f.render_widget(diff_paragraph, area);
                }
            }
            Err(_) => {
                let error_diff = Paragraph::new("error loading diff.")
                    .block(Block::default().borders(Borders::ALL).title("diff"))
                    .style(Style::default().fg(Color::Red));
                f.render_widget(error_diff, area);
            }
        }
        
    } else {
        let no_file = Paragraph::new("select a file to view diff.")
            .block(Block::default().borders(Borders::ALL).title("diff"))
            .style(Style::default().fg(Color::Gray));
        f.render_widget(no_file, area);
    }
}

fn draw_log_view(f: &mut Frame, area: Rect, app: &App){
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);
    
    draw_commit_list(f, chunks[0], app);
    
    draw_commit_details(f, chunks[1], app);
}

fn draw_commit_list(f: &mut Frame, area: Rect, app: &App){
    let items: Vec<ListItem> = app
        .commits
        .iter()
        .enumerate()
        .map(|(i, commit)| {
            let style = if i == app.selected_commit {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default().fg(Color::White)
            };
            
            let short_id = &commit.id[..8];
            let first_line = commit.message.lines().next().unwrap_or("");
            let time_str = commit.timestamp.format("%m/%d %H:%M").to_string();
            
            ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", short_id), Style::default().fg(Color::Cyan)),
                Span::styled(format!("{} ", first_line), style),
                Span::styled(format!("({})", time_str), Style::default().fg(Color::Gray)),
            ]))
        })
        .collect();
    
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("commit history."))
        .style(Style::default().fg(Color::White));
    
    f.render_widget(list, area);   
}

fn draw_commit_details(f: &mut Frame, area: Rect, app: &App){
    if let Some(commit) = app.commits.get(app.selected_commit) {
        let mut text = Vec::new();
        
        text.push(Line::from(vec![
            Span::styled("commit: ", Style::default().fg(Color::Gray)),
            Span::styled(&commit.id[..8], Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]));
        
        text.push(Line::from(vec![
            Span::styled("author: ", Style::default().fg(Color::Gray)),
            Span::styled(&commit.author, Style::default().fg(Color::White)),
        ]));
        
        text.push(Line::from(vec![
            Span::styled("date: ", Style::default().fg(Color::Gray)),
            Span::styled(commit.timestamp.format("%Y-%m-%d %H:%M:%S").to_string(), Style::default().fg(Color::White)),
        ]));
        
        text.push(Line::from(""));
        
        for line in commit.message.lines() {
            text.push(Line::from(line.to_string()));
        }
        
        if !commit.parents.is_empty(){
            text.push(Line::from(""));
            text.push(Line::from(vec![
                Span::styled("Parents: ", Style::default().fg(Color::Gray)),
            ]));
            for parent in &commit.parents {
                text.push(Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(&parent[..8], Style::default().fg(Color::Cyan)),
                ]));
            }
        }
        
        let paragraph = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title("commit details."))
            .wrap(Wrap { trim: true });
        
        f.render_widget(paragraph, area);
    } else {
        let empty = Paragraph::new("no commits found.")
            .block(Block::default().borders(Borders::ALL).title("commit details."))
            .style(Style::default().fg(Color::Gray));
        f.render_widget(empty, area);
    }
}


fn draw_branches_view(f: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .branches
        .iter()
        .enumerate()
        .map(|(i, branch)| {
            let is_selected = i == app.selected_file;
            let base_style = if branch.starts_with("origin/") {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::Green)
            };

            let style = if is_selected {
                base_style.bg(Color::DarkGray)
            } else {
                base_style
            };

            let name = if branch.starts_with("origin/") {
                format!("remote: {}", branch)
            } else {
                format!("local: {}", branch)
            };

            ListItem::new(Line::from(vec![
                Span::styled(name, style),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("branches (enter to checkout)"))
        .style(Style::default().fg(Color::White));

    f.render_widget(list, area);
}

fn draw_commit_dialog(f: &mut Frame, area: Rect, app: &App) {
    let popup_area = centered_rect(60, 20, area);

    f.render_widget(Clear, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3), Constraint::Length(3)])
        .split(popup_area);


    let title = Paragraph::new("commit message")
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)))
        .style(Style::default().fg(Color::White));
    f.render_widget(title, chunks[0]);

    let message = Paragraph::new(app.commit_message.as_str())
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::White).bg(Color::Black))
        .wrap(Wrap {trim: false});

    f.render_widget(message, chunks[1]);
    // test
    let help = Paragraph::new("enter: commit | esc: cancel")
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::Gray));

    f.render_widget(help, chunks[2]);

    f.set_cursor( // i am so cba to use the new one i am sorry
        chunks[1].x + app.commit_message.len() as u16 + 1,
        chunks[1].y + 1, // i do not know how make this wrap yet
    );

}

fn draw_error_popup(f: &mut Frame, area: Rect, error: &str) {
    let popup_area = centered_rect(50, 10, area);

    f.render_widget(Clear, popup_area);

    let error_widget = Paragraph::new(error)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("error")
                .border_style(Style::default().fg(Color::Red))
        )
        .style(Style::default().fg(Color::Red))
        .wrap(Wrap { trim: true });

    f.render_widget(error_widget, popup_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(100 - percent_y / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage(100 - percent_y / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(100 - percent_x / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage(100 - percent_x / 2),
        ])
        .split(popup_layout[1])[1]
}

fn draw_stash_view(f: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .stashes
        .iter()
        .enumerate()
        .map(|(i, stash)| {
            let style = if i == app.selected_stash {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default().fg(Color::White)
            };

            ListItem::new(Line::from(vec![
                Span::styled(stash, style),
            ]))        
        })  
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("stashes (enter: pop | del: drop | esc: back)"))
        .style(Style::default().fg(Color::White));

    f.render_widget(list, area);
}

fn draw_create_branch_dialog(f: &mut Frame, area: Rect, app: &App) {
    let popup_area = centered_rect(50, 15, area);

    f.render_widget(Clear, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(3), Constraint::Length(3)])
        .split(popup_area);

    let title = Paragraph::new("create new branch")
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Green)))
        .style(Style::default().fg(Color::White));

    f.render_widget(title, chunks[0]);

    let input = Paragraph::new(app.branch_name.as_str())
        .block(Block::default().borders(Borders::ALL).title("branch name"))
        .style(Style::default().fg(Color::White).bg(Color::Black));

    f.render_widget(input, chunks[1]);

    let help = Paragraph::new("enter: create | esc: cancel")
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::Gray));

    f.render_widget(help, chunks[2]);

    f.set_cursor(
        chunks[1].x + app.branch_name.len() as u16 + 1,
        chunks[1].y + 1,
    );
}

fn draw_stash_dialog(f: &mut Frame, area: Rect, app: &App) {
    let popup_area = centered_rect(60, 20, area);
    
    f.render_widget(Clear, popup_area);
    
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3), Constraint::Length(3)])
        .split(popup_area);
    
    let title = Paragraph::new("stash changes")
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)))
        .style(Style::default().fg(Color::White));
        
    f.render_widget(title, chunks[0]);
    
    let message = Paragraph::new(app.stash_message.as_str())
        .block(Block::default().borders(Borders::ALL).title("stash message (optional)"))
        .style(Style::default().fg(Color::White).bg(Color::Black))
        .wrap(Wrap { trim: false });

    f.render_widget(message, chunks[1]);
    
    let help = Paragraph::new("enter: stash | esc: cancel")
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::Gray));
        
    f.render_widget(help, chunks[2]);
    
    f.set_cursor(
        chunks[1].x + app.stash_message.len() as u16 + 1,
        chunks[1].y + 1,
    );
}

fn draw_remote_view(f: &mut Frame, area: Rect, app: &App){
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    
    draw_remote_list(f, chunks[0], app);
    
    draw_remote_operations(f, chunks[1], app);
}

fn draw_remote_list(f: &mut Frame, area: Rect, app: &App){
    let items: Vec<ListItem> = app
        .remotes
        .iter()
        .enumerate()
        .map(|(i, remote)| {
            let style = if i == app.selected_remote {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default().fg(Color::White)
            };
            
            ListItem::new(Line::from(vec![
                Span::styled(format!("remote: {}", remote), style),
            ]))
        })
        .collect();
    
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("remotes"))
        .style(Style::default().fg(Color::White));
    
    f.render_widget(list, area);
}

fn draw_remote_operations(f: &mut Frame, area: Rect, app: &App) {
    let mut text = Vec::new();
    
    text.push(Line::from(vec![
        Span::styled("current Branch: ", Style::default().fg(Color::Gray)),
        Span::styled(&app.current_branch, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
    ]));
    
    text.push(Line::from(""));
    
    if !app.remotes.is_empty() && app.selected_remote < app.remotes.len(){
        let selected_remote = &app.remotes[app.selected_remote];
        text.push(Line::from(vec![
            Span::styled("selected remote: ", Style::default().fg(Color::Gray)),
            Span::styled(selected_remote, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]));
        
        if let Ok(Some(url)) = app.repo.get_remote_url(selected_remote) {
            text.push(Line::from(vec![
                Span::styled("URL: ", Style::default().fg(Color::Gray)),
                Span::styled(url, Style::default().fg(Color::Blue)),
            ]));
        }
        
        text.push(Line::from(""));

        if app.is_pushing {
            text.push(Line::from(vec![
                Span::styled("pushing...", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ]));
        } else if app.is_pulling {
            text.push(Line::from(vec![
                Span::styled("pulling...", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ]));
        } else {
            text.push(Line::from(vec![
                Span::styled("operations:", Style::default().fg(Color::Gray)),
            ]));
            text.push(Line::from(vec![
                Span::styled("  p - Push to remote", Style::default().fg(Color::Green)),
            ]));
            text.push(Line::from(vec![
                Span::styled("  u - Pull from remote", Style::default().fg(Color::Blue)),
            ]));
        }
    } else {
        text.push(Line::from(vec![
            Span::styled("No remotes configured", Style::default().fg(Color::Red)),
        ]));
        text.push(Line::from(vec![
            Span::styled("use 'git remote add origin <url>' to add a remote", Style::default().fg(Color::Gray)),
        ]));
    }
    
    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("remote operations."))
        .wrap(Wrap { trim: true });
    
    f.render_widget(paragraph, area);
}