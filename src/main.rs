mod pueue_client;
mod ui;
#[cfg(test)]
mod tests;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    widgets::TableState,
    Terminal,
};
use std::io;
use std::time::{Duration, Instant};

use crate::pueue_client::{PueueClient, PueueClientOps};
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
    pueue_client: &mut impl PueueClientOps,
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
            ui::draw(f, &state, &mut table_state, &task_ids, jiff::Timestamp::now());
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
