mod pueue_client;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};
use std::io;
use std::time::{Duration, Instant};

use crate::pueue_client::PueueClient;
use pueue_lib::state::State;

#[tokio::main]
async fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut pueue_client = match PueueClient::new().await {
        Ok(client) => Some(client),
        Err(e) => {
            // Restore terminal before printing error
            disable_raw_mode()?;
            execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
            terminal.show_cursor()?;
            eprintln!("Failed to connect to Pueue daemon: {}", e);
            return Ok(());
        }
    };

    let res = run_app(&mut terminal, pueue_client.as_mut().unwrap()).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    pueue_client: &mut PueueClient,
) -> Result<()> {
    let mut last_tick = Instant::now();
    let tick_rate = Duration::from_millis(250);
    let mut state: Option<State> = None;
    let mut list_state = ListState::default();
    list_state.select(Some(0));

    loop {
        if state.is_none() || last_tick.elapsed() >= tick_rate {
            if let Ok(new_state) = pueue_client.get_state().await {
                state = Some(new_state);
            }
            last_tick = Instant::now();
        }

        let task_ids: Vec<usize> = state.as_ref()
            .map(|s| s.tasks.keys().cloned().collect())
            .unwrap_or_default();

        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(0),
                    Constraint::Length(3),
                ].as_ref())
                .split(f.area());

            let title_block = Block::default()
                .borders(Borders::ALL)
                .title(" Pui - Pueue TUI ");
            let title = Paragraph::new("j/k: Nav | s: Start | p: Pause | x: Kill | Backspace: Remove | q: Quit")
                .block(title_block);
            f.render_widget(title, chunks[0]);

            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(chunks[1]);

            if let Some(s) = &state {
                let tasks: Vec<ListItem> = s.tasks.iter().map(|(id, task)| {
                    let status = format!("{:?}", task.status);
                    let style = match status.as_str() {
                        "Running" => Style::default().fg(Color::Green),
                        "Queued" => Style::default().fg(Color::Yellow),
                        "Paused" => Style::default().fg(Color::Blue),
                        "Done" => Style::default().fg(Color::DarkGray),
                        _ => Style::default(),
                    };
                    let content = format!("[{}] {} - {}", id, task.command, status);
                    ListItem::new(content).style(style)
                }).collect();

                let task_list = List::new(tasks)
                    .block(Block::default().borders(Borders::ALL).title(" Tasks "))
                    .highlight_style(Style::default().add_modifier(Modifier::BOLD).bg(Color::Rgb(50, 50, 50)))
                    .highlight_symbol(">> ");
                f.render_stateful_widget(task_list, main_chunks[0], &mut list_state);

                // Task Details
                let selected_id = list_state.selected()
                    .and_then(|i| task_ids.get(i));

                let details_text = if let Some(id) = selected_id {
                    if let Some(task) = s.tasks.get(id) {
                        let mut details = format!(
                            "ID: {}\nCommand: {}\nPath: {}\nGroup: {}\nStatus: {:?}\n",
                            id, task.command, task.path.display(), task.group, task.status
                        );
                        if let Some(label) = &task.label {
                            details.push_str(&format!("Label: {}\n", label));
                        }
                        details
                    } else {
                        "Task not found".to_string()
                    }
                } else {
                    "No task selected".to_string()
                };

                let details = Paragraph::new(details_text)
                    .block(Block::default().borders(Borders::ALL).title(" Details "));
                f.render_widget(details, main_chunks[1]);
            } else {
                let loading = Paragraph::new("Loading state from Pueue...")
                    .block(Block::default().borders(Borders::ALL).title(" Tasks "));
                f.render_widget(loading, main_chunks[0]);
            }

            let footer = Paragraph::new("Connected to Pueue daemon")
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(footer, chunks[2]);
        })?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::from_secs(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('j') | KeyCode::Down => {
                        let i = match list_state.selected() {
                            Some(i) => {
                                if i >= task_ids.len() - 1 {
                                    0
                                } else {
                                    i + 1
                                }
                            }
                            None => 0,
                        };
                        list_state.select(Some(i));
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        let i = match list_state.selected() {
                            Some(i) => {
                                if i == 0 {
                                    task_ids.len() - 1
                                } else {
                                    i - 1
                                }
                            }
                            None => 0,
                        };
                        list_state.select(Some(i));
                    }
                    KeyCode::Char('s') => {
                        if let Some(i) = list_state.selected() {
                            if let Some(id) = task_ids.get(i) {
                                pueue_client.start_tasks(vec![*id]).await?;
                            }
                        }
                    }
                    KeyCode::Char('p') => {
                        if let Some(i) = list_state.selected() {
                            if let Some(id) = task_ids.get(i) {
                                pueue_client.pause_tasks(vec![*id]).await?;
                            }
                        }
                    }
                    KeyCode::Char('x') => {
                        if let Some(i) = list_state.selected() {
                            if let Some(id) = task_ids.get(i) {
                                pueue_client.kill_tasks(vec![*id]).await?;
                            }
                        }
                    }
                    KeyCode::Backspace => {
                        if let Some(i) = list_state.selected() {
                            if let Some(id) = task_ids.get(i) {
                                pueue_client.remove_tasks(vec![*id]).await?;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
