mod pueue_client;
#[cfg(test)]
mod tests;
mod ui;

use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind};
use futures::stream::StreamExt;
use ratatui::{DefaultTerminal, Frame, widgets::TableState};
use std::time::Duration;
use tokio::time::MissedTickBehavior;

use crate::pueue_client::{PueueClient, PueueClientOps};
use pueue_lib::message::TaskToRestart;
use pueue_lib::state::State;
use pueue_lib::task::TaskStatus;

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
    let app = App::new(pueue_client);
    let result = app.run(terminal).await;
    ratatui::restore();
    result
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortField {
    #[default]
    Id,
    Status,
    Command,
    Path,
}

#[derive(Debug)]
enum AppMode {
    Normal,
    Filter,
    Sort,
    Log(LogState),
}

#[derive(Debug)]
pub struct App<P: PueueClientOps> {
    /// Is the application running?
    running: bool,
    /// Event stream
    event_stream: EventStream,
    /// Pueue client
    pueue_client: P,
    /// Tick rate
    tick_rate: Duration,
    /// Pueue state
    pub(crate) state: Option<State>,
    /// Table state
    pub(crate) table_state: TableState,
    /// Selected task ID (to maintain selection across sort changes)
    pub(crate) selected_task_id: Option<usize>,
    /// Show details popup
    show_details: bool,
    /// Application mode
    app_mode: AppMode,
    /// Filter text
    filter_text: String,
    /// Sort field for task table
    pub(crate) sort_field: SortField,
    /// Connection status message for footer (e.g., "Not connected")
    connection_error: Option<String>,
    /// Error modal message (dismissible with Esc)
    error_modal: Option<String>,
}

impl<P: PueueClientOps> App<P> {
    /// Construct a new instance of [`App`].
    pub fn new(pueue_client: P) -> Self {
        let mut table_state = TableState::default();
        table_state.select(Some(0));

        Self {
            running: false,
            event_stream: EventStream::new(),
            pueue_client,
            tick_rate: Duration::from_millis(250),
            state: None,
            table_state,
            selected_task_id: None,
            show_details: false,
            app_mode: AppMode::Normal,
            filter_text: String::new(),
            sort_field: SortField::default(),
            connection_error: None,
            error_modal: None,
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
                    // Show connection errors in footer but keep running
                    match self.refresh_state().await {
                        Ok(()) => {
                            self.connection_error = None;
                        }
                        Err(e) => {
                            let err_str = e.to_string().to_lowercase();
                            if err_str.contains("broken pipe") || err_str.contains("connection") {
                                // Try to reconnect
                                self.connection_error = Some("Reconnecting to Pueue daemon...".to_string());
                                match self.pueue_client.reconnect().await {
                                    Ok(()) => {
                                        // Try to refresh state with new connection
                                        if let Err(e) = self.refresh_state().await {
                                            self.connection_error = Some(format!("Reconnection failed: {}", e));
                                        } else {
                                            self.connection_error = None;
                                        }
                                    }
                                    Err(e) => {
                                        self.connection_error = Some(format!("Reconnection failed: {}", e));
                                    }
                                }
                            } else {
                                self.connection_error = Some(format!("Error: {}", e));
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Renders the user interface.
    pub(crate) fn draw(&mut self, frame: &mut Frame) {
        let task_ids = self.get_sorted_task_ids(&self.filter_text, self.sort_field);

        // Sync table selection with selected_task_id
        self.sync_selection_with_task_id(&task_ids);

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
            sort_mode: matches!(self.app_mode, AppMode::Sort),
            sort_field: self.sort_field,
            log_view,
            connection_error: self.connection_error.as_deref(),
            error_modal: self.error_modal.as_deref(),
        };

        ui::draw(frame, &mut ui_state);
    }

    /// Sync table_state selection with selected_task_id
    /// This ensures selection follows the task across sort changes
    fn sync_selection_with_task_id(&mut self, task_ids: &[usize]) {
        if task_ids.is_empty() {
            self.table_state.select(None);
            self.selected_task_id = None;
            return;
        }

        // If we have a selected task ID, find its new row position
        if let Some(task_id) = self.selected_task_id {
            if let Some(row) = task_ids.iter().position(|&id| id == task_id) {
                self.table_state.select(Some(row));
            } else {
                // Task no longer exists (was filtered out or removed), select first
                self.table_state.select(Some(0));
                self.selected_task_id = Some(task_ids[0]);
            }
        } else {
            // No task selected yet, select first task
            self.table_state.select(Some(0));
            self.selected_task_id = Some(task_ids[0]);
        }
    }

    /// Update selected_task_id based on current table selection and task list
    pub(crate) fn update_selected_task_id(&mut self) {
        let task_ids = self.get_filtered_task_ids();
        if let Some(row) = self.table_state.selected() {
            self.selected_task_id = task_ids.get(row).copied();
        }
    }

    /// Handles the key events and updates the state of [`App`].
    async fn on_key_event(&mut self, key: KeyEvent) -> Result<()> {
        let mut next_mode = None;

        match &mut self.app_mode {
            AppMode::Filter => match key.code {
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
            },
            AppMode::Sort => match key.code {
                KeyCode::Esc => {
                    next_mode = Some(AppMode::Normal);
                }
                KeyCode::Char('i') => {
                    self.sort_field = SortField::Id;
                    next_mode = Some(AppMode::Normal);
                }
                KeyCode::Char('s') => {
                    self.sort_field = SortField::Status;
                    next_mode = Some(AppMode::Normal);
                }
                KeyCode::Char('c') => {
                    self.sort_field = SortField::Command;
                    next_mode = Some(AppMode::Normal);
                }
                KeyCode::Char('p') => {
                    self.sort_field = SortField::Path;
                    next_mode = Some(AppMode::Normal);
                }
                KeyCode::Char('q') => self.quit(),
                _ => {}
            },
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
                // Handle error modal first - Esc dismisses it
                if self.error_modal.is_some() {
                    match key.code {
                        KeyCode::Esc => self.error_modal = None,
                        KeyCode::Char('q') => self.quit(),
                        _ => {}
                    }
                } else if self.show_details {
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
                                            self.error_modal =
                                                Some(format!("Failed to start log stream: {}", e));
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
                            self.update_selected_task_id();
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
                            self.update_selected_task_id();
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
                                    self.table_state
                                        .select(Some(selected.saturating_sub(visible_rows)));
                                }
                                self.update_selected_task_id();
                            }
                        }
                        KeyCode::PageDown => {
                            let task_ids = self.get_filtered_task_ids();
                            if !task_ids.is_empty() {
                                let terminal_size = crossterm::terminal::size()?;
                                let visible_rows = terminal_size.1.saturating_sub(11) as usize;
                                let offset = self.table_state.offset();
                                let bottom = (offset + visible_rows)
                                    .saturating_sub(1)
                                    .min(task_ids.len().saturating_sub(1));
                                let selected = self.table_state.selected().unwrap_or(0);
                                if selected < bottom {
                                    self.table_state.select(Some(bottom));
                                } else {
                                    self.table_state.select(Some(
                                        (selected + visible_rows)
                                            .min(task_ids.len().saturating_sub(1)),
                                    ));
                                }
                                self.update_selected_task_id();
                            }
                        }
                        KeyCode::Home => {
                            let task_ids = self.get_filtered_task_ids();
                            if !task_ids.is_empty() {
                                self.table_state.select(Some(0));
                                self.update_selected_task_id();
                            }
                        }
                        KeyCode::End => {
                            let task_ids = self.get_filtered_task_ids();
                            if !task_ids.is_empty() {
                                self.table_state
                                    .select(Some(task_ids.len().saturating_sub(1)));
                                self.update_selected_task_id();
                            }
                        }
                        KeyCode::Char('f') => {
                            self.app_mode = AppMode::Filter;
                        }
                        KeyCode::Char('s') => {
                            self.app_mode = AppMode::Sort;
                        }
                        KeyCode::Char('r') => {
                            if let Some(i) = self.table_state.selected() {
                                let task_ids = self.get_filtered_task_ids();
                                if let Some(id) = task_ids.get(i) {
                                    let task_id = *id;
                                    // Check if task is Done (finished/failed) - needs restart instead of start
                                    let is_done = self
                                        .state
                                        .as_ref()
                                        .and_then(|s| s.tasks.get(&task_id))
                                        .is_some_and(|t| {
                                            matches!(t.status, TaskStatus::Done { .. })
                                        });

                                    let result = if is_done {
                                        // Restart the finished/failed task
                                        let task = self
                                            .state
                                            .as_ref()
                                            .unwrap()
                                            .tasks
                                            .get(&task_id)
                                            .unwrap();
                                        self.pueue_client
                                            .restart_tasks(vec![TaskToRestart {
                                                task_id,
                                                original_command: task.original_command.clone(),
                                                path: task.path.clone(),
                                                label: task.label.clone(),
                                                priority: task.priority,
                                            }])
                                            .await
                                    } else {
                                        self.pueue_client.start_tasks(vec![task_id]).await
                                    };

                                    if let Err(e) = result {
                                        self.error_modal =
                                            Some(format!("Failed to start task: {}", e));
                                    } else {
                                        let _ = self.refresh_state().await;
                                    }
                                }
                            }
                        }
                        KeyCode::Char('p') => {
                            if let Some(i) = self.table_state.selected() {
                                let task_ids = self.get_filtered_task_ids();
                                if let Some(id) = task_ids.get(i) {
                                    if let Err(e) = self.pueue_client.pause_tasks(vec![*id]).await {
                                        self.error_modal =
                                            Some(format!("Failed to pause task: {}", e));
                                    } else {
                                        let _ = self.refresh_state().await;
                                    }
                                }
                            }
                        }
                        KeyCode::Char('x') => {
                            if let Some(i) = self.table_state.selected() {
                                let task_ids = self.get_filtered_task_ids();
                                if let Some(id) = task_ids.get(i) {
                                    if let Err(e) = self.pueue_client.kill_tasks(vec![*id]).await {
                                        self.error_modal =
                                            Some(format!("Failed to kill task: {}", e));
                                    } else {
                                        let _ = self.refresh_state().await;
                                    }
                                }
                            }
                        }
                        KeyCode::Backspace => {
                            if let Some(i) = self.table_state.selected() {
                                let task_ids = self.get_filtered_task_ids();
                                if let Some(id) = task_ids.get(i) {
                                    let task_id = *id;
                                    // Don't remove running or paused tasks
                                    let is_active = self
                                        .state
                                        .as_ref()
                                        .and_then(|s| s.tasks.get(&task_id))
                                        .is_some_and(|t| {
                                            matches!(
                                                t.status,
                                                TaskStatus::Running { .. }
                                                    | TaskStatus::Paused { .. }
                                            )
                                        });

                                    if !is_active {
                                        if let Err(e) =
                                            self.pueue_client.remove_tasks(vec![task_id]).await
                                        {
                                            self.error_modal =
                                                Some(format!("Failed to remove task: {}", e));
                                        } else {
                                            let _ = self.refresh_state().await;
                                            let next_index = if i > 0 { i - 1 } else { 0 };
                                            self.table_state.select(Some(next_index));
                                            self.update_selected_task_id();
                                        }
                                    }
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

    /// Get filtered and sorted task IDs
    fn get_filtered_task_ids(&self) -> Vec<usize> {
        self.get_sorted_task_ids(&self.filter_text, self.sort_field)
    }

    /// Get task IDs filtered by the given filter text and sorted by the given field
    fn get_sorted_task_ids(&self, filter_text: &str, sort_field: SortField) -> Vec<usize> {
        self.state
            .as_ref()
            .map(|s| {
                let now = jiff::Timestamp::now();
                let mut ids: Vec<usize> = s
                    .tasks
                    .iter()
                    .filter(|(id, task)| {
                        ui::format_task(**id, task, &now).matches_filter(filter_text)
                    })
                    .map(|(id, _)| *id)
                    .collect();

                // Sort by the selected field
                ids.sort_by(|a, b| {
                    let task_a = s.tasks.get(a);
                    let task_b = s.tasks.get(b);
                    match (task_a, task_b) {
                        (Some(ta), Some(tb)) => match sort_field {
                            SortField::Id => a.cmp(b),
                            SortField::Status => {
                                let sa = ui::status_display(&ta.status);
                                let sb = ui::status_display(&tb.status);
                                sa.cmp(&sb).then_with(|| a.cmp(b))
                            }
                            SortField::Command => {
                                ta.command.cmp(&tb.command).then_with(|| a.cmp(b))
                            }
                            SortField::Path => ta.path.cmp(&tb.path).then_with(|| a.cmp(b)),
                        },
                        _ => a.cmp(b),
                    }
                });
                ids
            })
            .unwrap_or_default()
    }

    /// Refresh the state immediately from the pueue client
    async fn refresh_state(&mut self) -> Result<()> {
        let new_state = self.pueue_client.get_state().await?;
        self.state = Some(new_state);
        Ok(())
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
            .field(
                "stream_client",
                &self.stream_client.as_ref().map(|_| "Some(...)"),
            )
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

    pub(crate) fn handle_key(&mut self, key: KeyEvent, page_height: u16, page_width: u16) -> bool {
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
            let max_offset = self
                .visual_line_count(page_width)
                .saturating_sub(page_height);
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

    pub(crate) fn update_autoscroll(&mut self, page_height: u16, page_width: u16) {
        if self.autoscroll {
            let lines = self.visual_line_count(page_width);
            self.scroll_offset = lines.saturating_sub(page_height);
        }
    }
}
