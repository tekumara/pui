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
    widgets::{Block, Borders, Cell, Row, Table, TableState, Paragraph},
    Terminal,
};
use std::io;
use std::time::{Duration, Instant};
use std::path::Path;

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
    let mut table_state = TableState::default();
    table_state.select(Some(0));

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
                let rows: Vec<Row> = s.tasks.iter().map(|(id, task)| {
                    let status = format!("{:?}", task.status);
                    let style = match status.as_str() {
                        "Running" => Style::default().fg(Color::Green),
                        "Queued" => Style::default().fg(Color::Yellow),
                        "Paused" => Style::default().fg(Color::Blue),
                        "Done" => Style::default().fg(Color::DarkGray),
                        _ => Style::default(),
                    };

                    let command_basename = Path::new(&task.command)
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or(&task.command);

                    let shortened_path = tico::tico(&task.path.to_string_lossy());

                    let (start, end) = task.start_and_end();
                    let duration_str = if let Some(start) = start {
                        let start_ts = jiff::Timestamp::from_second(start.timestamp()).unwrap();
                        let end_ts = if let Some(end) = end {
                            jiff::Timestamp::from_second(end.timestamp()).unwrap()
                        } else {
                            jiff::Timestamp::now()
                        };
                        let duration = end_ts.duration_since(start_ts);

                        if duration.as_secs() < 60 {
                            format!("{}s", duration.as_secs())
                        } else if duration.as_secs() < 3600 {
                            format!("{}m {}s", duration.as_secs() / 60, duration.as_secs() % 60)
                        } else {
                            format!("{}h {}m", duration.as_secs() / 3600, (duration.as_secs() % 3600) / 60)
                        }
                    } else {
                        "-".to_string()
                    };

                    Row::new(vec![
                        Cell::from(id.to_string()),
                        Cell::from(status),
                        Cell::from(command_basename.to_string()),
                        Cell::from(shortened_path),
                        Cell::from(duration_str),
                    ]).style(style)
                }).collect();

                let header = Row::new(vec!["Id", "Status", "Command", "Path", "Duration"])
                    .style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan));

                let task_table = Table::new(rows, [
                    Constraint::Length(4),
                    Constraint::Length(10),
                    Constraint::Percentage(30),
                    Constraint::Percentage(30),
                    Constraint::Length(10),
                ])
                .header(header)
                .block(Block::default().borders(Borders::ALL).title(" Tasks "))
                .row_highlight_style(Style::default().add_modifier(Modifier::BOLD).bg(Color::Rgb(50, 50, 50)))
                .highlight_symbol(">> ");

                f.render_stateful_widget(task_table, main_chunks[0], &mut table_state);

                // Task Details
                let selected_id = table_state.selected()
                    .and_then(|i| task_ids.get(i));

                let details_text = if let Some(id) = selected_id {
                    if let Some(task) = s.tasks.get(id) {
                        let (start, end) = task.start_and_end();
                        let duration_str = if let Some(start) = start {
                            let start_ts = jiff::Timestamp::from_second(start.timestamp()).unwrap();
                            let end_ts = if let Some(end) = end {
                                jiff::Timestamp::from_second(end.timestamp()).unwrap()
                            } else {
                                jiff::Timestamp::now()
                            };
                            let duration = end_ts.duration_since(start_ts);
                            if duration.as_secs() < 60 {
                                format!("{}s", duration.as_secs())
                            } else if duration.as_secs() < 3600 {
                                format!("{}m {}s", duration.as_secs() / 60, duration.as_secs() % 60)
                            } else {
                                format!("{}h {}m", duration.as_secs() / 3600, (duration.as_secs() % 3600) / 60)
                            }
                        } else {
                            "-".to_string()
                        };

                        let command_basename = Path::new(&task.command)
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or(&task.command);

                        let shortened_path = tico::tico(&task.path.to_string_lossy());

                        let mut details = format!(
                            "ID: {}\nStatus: {:?}\nCommand: {}\nPath: {}\nDuration: {}\nGroup: {}\n",
                            id, task.status, command_basename, shortened_path, duration_str, task.group
                        );
                        if let Some(label) = &task.label {
                            details.push_str(&format!("Label: {}\n", label));
                        }
                        details.push_str(&format!("\nFull Command: {}\nFull Path: {}\n", task.command, task.path.display()));
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

        // Calculate how much time is left until the next scheduled refresh (250ms).
        // This ensures the TUI polls for input but still refreshes the state on schedule.
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::from_secs(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('j') | KeyCode::Down => {
                        let i = match table_state.selected() {
                            Some(i) => {
                                if i >= task_ids.len() - 1 {
                                    0
                                } else {
                                    i + 1
                                }
                            }
                            None => 0,
                        };
                        table_state.select(Some(i));
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        let i = match table_state.selected() {
                            Some(i) => {
                                if i == 0 {
                                    task_ids.len() - 1
                                } else {
                                    i - 1
                                }
                            }
                            None => 0,
                        };
                        table_state.select(Some(i));
                    }
                    KeyCode::Char('s') => {
                        if let Some(i) = table_state.selected() {
                            if let Some(id) = task_ids.get(i) {
                                pueue_client.start_tasks(vec![*id]).await?;
                            }
                        }
                    }
                    KeyCode::Char('p') => {
                        if let Some(i) = table_state.selected() {
                            if let Some(id) = task_ids.get(i) {
                                pueue_client.pause_tasks(vec![*id]).await?;
                            }
                        }
                    }
                    KeyCode::Char('x') => {
                        if let Some(i) = table_state.selected() {
                            if let Some(id) = task_ids.get(i) {
                                pueue_client.kill_tasks(vec![*id]).await?;
                            }
                        }
                    }
                    KeyCode::Backspace => {
                        if let Some(i) = table_state.selected() {
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
