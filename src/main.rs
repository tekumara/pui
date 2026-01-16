mod pueue_client;
mod ui;
#[cfg(test)]
mod tests;

use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind};
use futures::StreamExt;
use ratatui::{
    widgets::TableState,
    DefaultTerminal, Frame,
};
use std::time::Duration;
use tokio::time::MissedTickBehavior;

use crate::pueue_client::{PueueClient, PueueClientOps};
use pueue_lib::state::State;

#[tokio::main]
async fn main() -> Result<()> {
    let terminal = ratatui::init();
    let pueue_client = match PueueClient::new().await {
        Ok(client) => client,
        Err(e) => {
            ratatui::restore();
            eprintln!("Failed to connect to Pueue daemon: {}", e);
            return Ok(());
        }
    };
    let result = App::new(pueue_client).run(terminal).await;
    ratatui::restore();
    result
}

#[derive(Debug)]
enum AppMode {
    Normal,
    Filter,
    Log(LogState),
}

#[derive(Debug)]
pub struct App {
    /// Is the application running?
    running: bool,
    /// Event stream
    event_stream: EventStream,
    /// Pueue client
    pueue_client: PueueClient,
    /// Tick rate
    tick_rate: Duration,
    /// Pueue state
    state: Option<State>,
    /// Table state
    table_state: TableState,
    /// Show details popup
    show_details: bool,
    /// Application mode
    app_mode: AppMode,
    /// Filter text
    filter_text: String,
}

impl App {
    /// Construct a new instance of [`App`].
    pub fn new(pueue_client: PueueClient) -> Self {
        let mut table_state = TableState::default();
        table_state.select(Some(0));

        Self {
            running: false,
            event_stream: EventStream::new(),
            pueue_client,
            tick_rate: Duration::from_millis(250),
            state: None,
            table_state,
            show_details: false,
            app_mode: AppMode::Normal,
            filter_text: String::new(),
        }
    }

    /// Run the application's main loop using select! to handle multiple async sources.
    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.running = true;
        let mut tick_interval = tokio::time::interval(self.tick_rate);
        tick_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        while self.running {
            // Draw the UI
            terminal.draw(|frame| self.draw(frame))?;

            tokio::select! {

                // Handle keyboard/terminal events
                event = self.event_stream.next() => {
                    if let Some(Ok(evt)) = event {
                        match evt {
                            Event::Key(key) if key.kind == KeyEventKind::Press => {
                                self.on_key_event(key).await?;
                            }
                            Event::Mouse(_) | Event::Resize(_, _) => {}
                            _ => {}
                        }
                    }
                }

                // Stream logs when in Log mode with an active stream
                // Chunks are produced once every 1000ms by the pueue client
                chunk_result = async {
                    if let AppMode::Log(log_state) = &mut self.app_mode
                        && let Some(stream_client) = &mut log_state.stream_client
                    {
                        return Some(stream_client.receive_stream_chunk().await);
                    }
                    // If not in log mode or no stream, pend forever (let other branches have a turn)
                    std::future::pending::<Option<Result<Option<String>>>>().await
                } => {
                    if let Some(result) = chunk_result
                        && let AppMode::Log(log_state) = &mut self.app_mode
                    {
                        match result {
                            Ok(Some(chunk)) => {
                                if !chunk.is_empty() {
                                    log_state.logs.push_str(&chunk);
                                    // Update autoscroll if enabled
                                    if log_state.autoscroll {
                                        let terminal_size = crossterm::terminal::size()?;
                                        let page_height = terminal_size.1.saturating_sub(2);
                                        let page_width = terminal_size.0.saturating_sub(2);
                                        log_state.update_autoscroll(page_height, page_width);
                                    }
                                }
                            }
                            Ok(None) => {
                                // Stream closed (task finished)
                                log_state.stream_client = None;
                            }
                            Err(_) => {
                                // Error receiving chunk, close the stream
                                log_state.stream_client = None;
                            }
                        }
                    }
                }

                // Tick timeout for state refresh
                _ = tick_interval.tick() => {
                    // Fetch pueue state on tick
                    if let Ok(new_state) = self.pueue_client.get_state().await {
                        self.state = Some(new_state);
                    }
                }
            }
        }
        Ok(())
    }

    /// Renders the user interface.
    fn draw(&mut self, frame: &mut Frame) {
        let task_ids: Vec<usize> = self.state.as_ref()
            .map(|s| {
                let now = jiff::Timestamp::now();
                let mut ids: Vec<usize> = s.tasks.iter()
                    .filter(|(id, task)| {
                        ui::format_task(**id, task, &now).matches_filter(&self.filter_text)
                    })
                    .map(|(id, _)| *id)
                    .collect();
                ids.sort();
                ids
            })
            .unwrap_or_default();

        let log_view = if let AppMode::Log(log_state) = &self.app_mode {
            Some((log_state.logs.as_str(), log_state.scroll_offset))
        } else {
            None
        };

        let mut ui_state = ui::UiState {
            state: &self.state,
            table_state: &mut self.table_state,
            task_ids: &task_ids,
            now: jiff::Timestamp::now(),
            show_details: self.show_details,
            filter_text: &self.filter_text,
            input_mode: matches!(self.app_mode, AppMode::Filter),
            log_view,
        };

        ui::draw(frame, &mut ui_state);
    }

    /// Handles the key events and updates the state of [`App`].
    async fn on_key_event(&mut self, key: KeyEvent) -> Result<()> {
        let mut next_mode = None;

        match &mut self.app_mode {
            AppMode::Filter => {
                match key.code {
                    KeyCode::Esc => {
                        next_mode = Some(AppMode::Normal);
                        self.filter_text.clear();
                    }
                    KeyCode::Enter => {
                        next_mode = Some(AppMode::Normal);
                    }
                    KeyCode::Backspace => {
                        self.filter_text.pop();
                    }
                    KeyCode::Char(c) => {
                        self.filter_text.push(c);
                    }
                    _ => {}
                }
            }
            AppMode::Log(log_state) => {
                let terminal_size = crossterm::terminal::size()?;
                let page_height = terminal_size.1.saturating_sub(2);
                let page_width = terminal_size.0.saturating_sub(2);

                if key.code == KeyCode::Esc {
                    // Drop the stream client when exiting log mode
                    next_mode = Some(AppMode::Normal);
                } else {
                    log_state.handle_key(key, page_height, page_width);
                }

                log_state.update_autoscroll(page_height, page_width);
            }
            AppMode::Normal => {
                if self.show_details {
                    match key.code {
                        KeyCode::Esc => self.show_details = false,
                        KeyCode::Char('q') => self.quit(),
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char('q') => self.quit(),
                        KeyCode::Esc => {
                            if !self.filter_text.is_empty() {
                                self.filter_text.clear();
                            }
                        }
                        KeyCode::Enter => {
                            if let Some(i) = self.table_state.selected() {
                                let task_ids = self.get_filtered_task_ids();
                                if let Some(id) = task_ids.get(i) {
                                    let task_id = *id;
                                    // Create streaming client and start the stream
                                    match Self::start_log_stream(task_id).await {
                                        Ok(log_state) => {
                                            next_mode = Some(AppMode::Log(log_state));
                                        }
                                        Err(e) => {
                                            eprintln!("Failed to start log stream: {:?}", e);
                                        }
                                    }
                                }
                            }
                        }
                        KeyCode::Char('d') => {
                            if self.table_state.selected().is_some() {
                                self.show_details = true;
                            }
                        }
                        KeyCode::Char('j') | KeyCode::Down => {
                            let task_ids = self.get_filtered_task_ids();
                            let i = match self.table_state.selected() {
                                Some(i) if !task_ids.is_empty() => {
                                    // Wrap to the top when moving down from the last row.
                                    if i >= task_ids.len().saturating_sub(1) {
                                        0
                                    } else {
                                        i + 1
                                    }
                                }
                                _ => 0,
                            };
                            self.table_state.select(Some(i));
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            let task_ids = self.get_filtered_task_ids();
                            let i = match self.table_state.selected() {
                                Some(i) if !task_ids.is_empty() => {
                                    // Wrap to the bottom when moving up from the first row.
                                    if i == 0 {
                                        task_ids.len().saturating_sub(1)
                                    } else {
                                        i - 1
                                    }
                                }
                                _ => 0,
                            };
                            self.table_state.select(Some(i));
                        }
                        KeyCode::PageUp => {
                            let task_ids = self.get_filtered_task_ids();
                            if !task_ids.is_empty() {
                                let offset = self.table_state.offset();
                                let selected = self.table_state.selected().unwrap_or(0);
                                if selected > offset {
                                    self.table_state.select(Some(offset));
                                } else {
                                    let terminal_size = crossterm::terminal::size()?;
                                    let visible_rows = terminal_size.1.saturating_sub(11) as usize;
                                    self.table_state.select(Some(selected.saturating_sub(visible_rows)));
                                }
                            }
                        }
                        KeyCode::PageDown => {
                            let task_ids = self.get_filtered_task_ids();
                            if !task_ids.is_empty() {
                                let terminal_size = crossterm::terminal::size()?;
                                let visible_rows = terminal_size.1.saturating_sub(11) as usize;
                                let offset = self.table_state.offset();
                                let bottom = (offset + visible_rows).saturating_sub(1).min(task_ids.len().saturating_sub(1));
                                let selected = self.table_state.selected().unwrap_or(0);
                                if selected < bottom {
                                    self.table_state.select(Some(bottom));
                                } else {
                                    self.table_state.select(Some((selected + visible_rows).min(task_ids.len().saturating_sub(1))));
                                }
                            }
                        }
                        KeyCode::Home => {
                            let task_ids = self.get_filtered_task_ids();
                            if !task_ids.is_empty() {
                                self.table_state.select(Some(0));
                            }
                        }
                        KeyCode::End => {
                            let task_ids = self.get_filtered_task_ids();
                            if !task_ids.is_empty() {
                                self.table_state.select(Some(task_ids.len().saturating_sub(1)));
                            }
                        }
                        KeyCode::Char('f') => {
                            self.app_mode = AppMode::Filter;
                        }
                        KeyCode::Char('s') => {
                            if let Some(i) = self.table_state.selected() {
                                let task_ids = self.get_filtered_task_ids();
                                if let Some(id) = task_ids.get(i) {
                                    self.pueue_client.start_tasks(vec![*id]).await?;
                                }
                            }
                        }
                        KeyCode::Char('p') => {
                            if let Some(i) = self.table_state.selected() {
                                let task_ids = self.get_filtered_task_ids();
                                if let Some(id) = task_ids.get(i) {
                                    self.pueue_client.pause_tasks(vec![*id]).await?;
                                }
                            }
                        }
                        KeyCode::Char('x') => {
                            if let Some(i) = self.table_state.selected() {
                                let task_ids = self.get_filtered_task_ids();
                                if let Some(id) = task_ids.get(i) {
                                    self.pueue_client.kill_tasks(vec![*id]).await?;
                                }
                            }
                        }
                        KeyCode::Backspace => {
                            if let Some(i) = self.table_state.selected() {
                                let task_ids = self.get_filtered_task_ids();
                                if let Some(id) = task_ids.get(i) {
                                    self.pueue_client.remove_tasks(vec![*id]).await?;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        if let Some(mode) = next_mode {
            self.app_mode = mode;
    }

    Ok(())
}

    /// Get filtered task IDs
    fn get_filtered_task_ids(&self) -> Vec<usize> {
        self.state.as_ref()
            .map(|s| {
                let now = jiff::Timestamp::now();
                let mut ids: Vec<usize> = s.tasks.iter()
                    .filter(|(id, task)| {
                        ui::format_task(**id, task, &now).matches_filter(&self.filter_text)
                    })
                    .map(|(id, _)| *id)
                    .collect();
                ids.sort();
                ids
            })
            .unwrap_or_default()
    }

    /// Set running to false to quit the application.
    fn quit(&mut self) {
        self.running = false;
    }

    /// Create a new streaming client and start the log stream
    async fn start_log_stream(task_id: usize) -> Result<LogState> {
        let mut stream_client = PueueClient::new().await?;
        let initial_logs = stream_client.start_log_stream(task_id, None).await?;

        let mut log_state = LogState {
            task_id,
            logs: initial_logs,
            scroll_offset: 0,
            autoscroll: true,
            stream_client: Some(stream_client),
        };

        // Scroll to end of initial logs
        if let Ok(terminal_size) = crossterm::terminal::size() {
            let page_height = terminal_size.1.saturating_sub(2);
            let page_width = terminal_size.0.saturating_sub(2);
            log_state.update_autoscroll(page_height, page_width);
        }

        Ok(log_state)
    }
}

pub struct LogState {
    pub task_id: usize,
    pub logs: String,
    pub scroll_offset: u16,
    pub autoscroll: bool,
    /// Streaming client - None if stream has closed
    pub stream_client: Option<PueueClient>,
}

impl std::fmt::Debug for LogState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LogState")
            .field("task_id", &self.task_id)
            .field("logs", &format!("({} bytes)", self.logs.len()))
            .field("scroll_offset", &self.scroll_offset)
            .field("autoscroll", &self.autoscroll)
            .field("stream_client", &self.stream_client.as_ref().map(|_| "Some(...)"))
            .finish()
    }
}

impl LogState {
    pub fn new(task_id: usize) -> Self {
        Self {
            task_id,
            logs: String::new(),
            scroll_offset: 0,
            autoscroll: true,
            stream_client: None,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent, page_height: u16, page_width: u16) -> bool {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
                self.autoscroll = false;
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
                self.autoscroll = false;
            }
            KeyCode::PageUp | KeyCode::Char('b') => {
                self.scroll_offset = self.scroll_offset.saturating_sub(page_height);
                self.autoscroll = false;
            }
            KeyCode::PageDown | KeyCode::Char(' ') => {
                self.scroll_offset = self.scroll_offset.saturating_add(page_height);
                self.autoscroll = false;
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.scroll_offset = 0;
                self.autoscroll = false;
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.autoscroll = true;
                self.update_autoscroll(page_height, page_width);
            }
            KeyCode::Char('d') => {
                self.scroll_offset = self.scroll_offset.saturating_add(page_height / 2);
                self.autoscroll = false;
            }
            KeyCode::Char('u') => {
                self.scroll_offset = self.scroll_offset.saturating_sub(page_height / 2);
                self.autoscroll = false;
            }
            _ => return false,
        }

        // Clamp manual scrolling to the last possible offset, so we can't overscroll into blank space.
        if !self.autoscroll {
            let max_offset = self.visual_line_count(page_width).saturating_sub(page_height);
            self.scroll_offset = self.scroll_offset.min(max_offset);
        }
        true
    }

    fn visual_line_count(&self, page_width: u16) -> u16 {
        // Use ratatui's own wrapping algorithm (Paragraph::line_count) so our autoscroll matches
        // exactly what gets rendered.
        use ratatui::widgets::{Paragraph, Wrap};

        let width = page_width.max(1);
        let logs = self.logs.replace('\t', "        ");
        let p = Paragraph::new(logs).wrap(Wrap { trim: false });
        p.line_count(width) as u16
    }

    pub fn update_autoscroll(&mut self, page_height: u16, page_width: u16) {
        if self.autoscroll {
            let lines = self.visual_line_count(page_width);
            self.scroll_offset = lines.saturating_sub(page_height);
        }
    }
}

