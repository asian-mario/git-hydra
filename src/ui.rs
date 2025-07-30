use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
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
    }

    draw_footer(f, chunks[2]);
}

fn draw_header(f: &mut Frame, area: Rect, app: &App){
    let titles = vec!["status (1)", "log (2)", "branches (3)"];
    let selected = match app.mode {
        AppMode::Status => 0,
        AppMode::Log => 1,
        AppMode::Branches => 2,
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
    let help_text = match f.area().width > 80 {
        true => "↑/↓: navigate | 1/2/3: switch tabs | Enter/Space: stage/unstage | F5: refresh | q: quit",
        false => "↑/↓: nav | 1/2/3: tabs | F5: refresh | q: quit",
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
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area);

        draw_file_changes(f, chunks[0], app, status);

        draw_repo_info(f, chunks[1], status);
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

// Test

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