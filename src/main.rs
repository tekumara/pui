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

enum AppMode {
    Normal,
    Filter,
    Log {
        task_id: usize,
        logs: String,
        scroll_offset: u16,
        autoscroll: bool,
    },
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
    let mut show_details = false;
    let mut app_mode = AppMode::Normal;
    let mut filter_text = String::new();

    loop {
        let should_fetch = state.is_none() || last_tick.elapsed() >= tick_rate;
        if should_fetch {
            if let Ok(new_state) = pueue_client.get_state().await {
                state = Some(new_state);
            }

            // Fetch logs if in Log mode
            if let AppMode::Log { task_id, logs, .. } = &mut app_mode {
                if let Ok(Some(new_logs)) = pueue_client.get_task_log(*task_id).await {
                    *logs = new_logs;
                }
            }

            last_tick = Instant::now();
        }

        let task_ids: Vec<usize> = state.as_ref()
            .map(|s| {
                let now = jiff::Timestamp::now();
                let mut ids: Vec<usize> = s.tasks.iter()
                    .filter(|(id, task)| {
                        ui::format_task(**id, task, &now).matches_filter(&filter_text)
                    })
                    .map(|(id, _)| *id)
                    .collect();
                ids.sort();
                ids
            })
            .unwrap_or_default();

        terminal.draw(|f| {
            let log_view = if let AppMode::Log { logs, scroll_offset, .. } = &app_mode {
                Some((logs.as_str(), *scroll_offset))
            } else {
                None
            };

            ui::draw(
                f,
                &state,
                &mut table_state,
                &task_ids,
                jiff::Timestamp::now(),
                show_details,
                &filter_text,
                matches!(app_mode, AppMode::Filter),
                log_view,
            );
        })?;

        // Calculate how much time is left until the next scheduled refresh (250ms).
        // This ensures the TUI polls for input but still refreshes the state on schedule.
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::from_secs(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                let mut next_mode = None;
                match &mut app_mode {
                    AppMode::Filter => {
                         match key.code {
                             KeyCode::Esc => {
                                 next_mode = Some(AppMode::Normal);
                                 filter_text.clear();
                             }
                             KeyCode::Enter => {
                                 next_mode = Some(AppMode::Normal);
                             }
                             KeyCode::Backspace => {
                                 filter_text.pop();
                             }
                             KeyCode::Char(c) => {
                                 filter_text.push(c);
                             }
                             _ => {}
                         }
                    }
                    AppMode::Log { logs, scroll_offset, autoscroll, .. } => {
                        let terminal_height = terminal.size()?.height;
                        let page_height = terminal_height.saturating_sub(2);

                        match key.code {
                            KeyCode::Esc => next_mode = Some(AppMode::Normal),
                            KeyCode::Char('j') | KeyCode::Down => {
                                *scroll_offset = scroll_offset.saturating_add(1);
                                *autoscroll = false;
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                *scroll_offset = scroll_offset.saturating_sub(1);
                                *autoscroll = false;
                            }
                            KeyCode::PageUp => {
                                *scroll_offset = scroll_offset.saturating_sub(page_height);
                                *autoscroll = false;
                            }
                            KeyCode::PageDown => {
                                *scroll_offset = scroll_offset.saturating_add(page_height);
                                *autoscroll = false;
                            }
                            KeyCode::Home => {
                                *scroll_offset = 0;
                                *autoscroll = false;
                            }
                            KeyCode::End => {
                                *autoscroll = true;
                            }
                            _ => {}
                        }

                        if *autoscroll {
                            let lines = logs.lines().count() as u16;
                            *scroll_offset = lines.saturating_sub(page_height);
                        }
                    }
                    AppMode::Normal => {
                        if show_details {
                             match key.code {
                                 KeyCode::Esc => show_details = false,
                                 KeyCode::Char('q') => return Ok(()),
                                 _ => {}
                             }
                        } else {
                            match key.code {
                                KeyCode::Char('q') => return Ok(()),
                                KeyCode::Esc => {
                                    if !filter_text.is_empty() {
                                        filter_text.clear();
                                    }
                                }
                                KeyCode::Enter => {
                                    if let Some(i) = table_state.selected() {
                                        if let Some(id) = task_ids.get(i) {
                                            next_mode = Some(AppMode::Log {
                                                task_id: *id,
                                                logs: String::new(),
                                                scroll_offset: 0,
                                                autoscroll: true,
                                            });
                                        }
                                    }
                                }
                                KeyCode::Char('d') => {
                                    if table_state.selected().is_some() {
                                        show_details = true;
                                    }
                                }
                                KeyCode::Char('j') | KeyCode::Down => {
                                    let i = match table_state.selected() {
                                        Some(i) if !task_ids.is_empty() => {
                                            if i >= task_ids.len().saturating_sub(1) {
                                                i
                                            } else {
                                                i + 1
                                            }
                                        }
                                        _ => 0,
                                    };
                                    table_state.select(Some(i));
                                }
                                KeyCode::Char('k') | KeyCode::Up => {
                                    let i = match table_state.selected() {
                                        Some(i) if !task_ids.is_empty() => {
                                            if i == 0 {
                                                0
                                            } else {
                                                i - 1
                                            }
                                        }
                                        _ => 0,
                                    };
                                    table_state.select(Some(i));
                                }
                                KeyCode::PageUp => {
                                    if !task_ids.is_empty() {
                                        let offset = table_state.offset();
                                        let selected = table_state.selected().unwrap_or(0);
                                        if selected > offset {
                                            table_state.select(Some(offset));
                                        } else {
                                            let terminal_size = terminal.size()?;
                                            let visible_rows = terminal_size.height.saturating_sub(11) as usize;
                                            table_state.select(Some(selected.saturating_sub(visible_rows)));
                                        }
                                    }
                                }
                                KeyCode::PageDown => {
                                    if !task_ids.is_empty() {
                                        let terminal_size = terminal.size()?;
                                        let visible_rows = terminal_size.height.saturating_sub(11) as usize;
                                        let offset = table_state.offset();
                                        let bottom = (offset + visible_rows).saturating_sub(1).min(task_ids.len().saturating_sub(1));
                                        let selected = table_state.selected().unwrap_or(0);
                                        if selected < bottom {
                                            table_state.select(Some(bottom));
                                        } else {
                                            table_state.select(Some((selected + visible_rows).min(task_ids.len().saturating_sub(1))));
                                        }
                                    }
                                }
                                KeyCode::Home => {
                                    if !task_ids.is_empty() {
                                        table_state.select(Some(0));
                                    }
                                }
                                KeyCode::End => {
                                    if !task_ids.is_empty() {
                                        table_state.select(Some(task_ids.len().saturating_sub(1)));
                                    }
                                }
                                KeyCode::Char('f') => {
                                    app_mode = AppMode::Filter;
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

                if let Some(mode) = next_mode {
                    app_mode = mode;
                }
            }
        }
    }
}
