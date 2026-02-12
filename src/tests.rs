use anyhow::Result;
use chrono::{Local, TimeZone};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pueue_lib::state::State;
use pueue_lib::task::{Task, TaskResult, TaskStatus};
use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    layout::Rect,
    widgets::TableState,
};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;

use crate::LogState;
use crate::SortField;
use crate::config::Config;
use crate::pueue_client::PueueClientOps;
use crate::ui;
use pueue_lib::message::TaskToRestart;

pub struct MockPueueClient {
    state: State,
}

impl MockPueueClient {
    pub fn new() -> Self {
        let mut state = State::default();

        // Fixed time for snapshot stability: 2026-01-01 00:00:00 (Local)
        let now = Local.timestamp_opt(1767225600, 0).unwrap();

        let task1 = Task {
            id: 0,
            created_at: now,
            original_command: "sleep 60".to_string(),
            command: "sleep 60".to_string(),
            path: PathBuf::from("/tmp"),
            envs: HashMap::new(),
            group: "default".to_string(),
            dependencies: vec![],
            priority: 0,
            label: None,
            status: TaskStatus::Running {
                enqueued_at: now,
                start: now,
            },
        };

        let task2 = Task {
            id: 1,
            created_at: now,
            original_command: "echo 'hello'".to_string(),
            command: "echo 'hello'".to_string(),
            path: PathBuf::from("/home/user"),
            envs: HashMap::new(),
            group: "default".to_string(),
            dependencies: vec![],
            priority: 0,
            label: None,
            status: TaskStatus::Done {
                enqueued_at: now,
                start: now,
                end: now,
                result: TaskResult::Success,
            },
        };

        let task3 = Task {
            id: 2,
            created_at: now,
            original_command: "false".to_string(),
            command: "false".to_string(),
            path: PathBuf::from("/tmp"),
            envs: HashMap::new(),
            group: "default".to_string(),
            dependencies: vec![],
            priority: 0,
            label: None,
            status: TaskStatus::Done {
                enqueued_at: now,
                start: now,
                end: now,
                result: TaskResult::Failed(1),
            },
        };

        state.tasks.insert(0, task1);
        state.tasks.insert(1, task2);
        state.tasks.insert(2, task3);

        Self { state }
    }
}

impl PueueClientOps for MockPueueClient {
    async fn get_state(&mut self) -> Result<State> {
        Ok(self.state.clone())
    }

    async fn start_tasks(&mut self, _ids: Vec<usize>) -> Result<()> {
        Ok(())
    }

    async fn restart_tasks(&mut self, _tasks: Vec<TaskToRestart>) -> Result<()> {
        Ok(())
    }

    async fn enqueue_tasks(&mut self, _ids: Vec<usize>) -> Result<()> {
        Ok(())
    }

    async fn pause_tasks(&mut self, _ids: Vec<usize>) -> Result<()> {
        Ok(())
    }

    async fn kill_tasks(&mut self, _ids: Vec<usize>) -> Result<()> {
        Ok(())
    }

    async fn remove_tasks(&mut self, _ids: Vec<usize>) -> Result<()> {
        Ok(())
    }

    async fn start_log_stream(&mut self, _id: usize, _lines: Option<usize>) -> Result<String> {
        Ok("Log line 1\nLog line 2\nLog line 3".to_string())
    }

    async fn receive_stream_chunk(&mut self) -> Result<Option<String>> {
        Ok(None) // Immediately close the stream for tests
    }

    async fn reconnect(&mut self) -> Result<()> {
        Ok(())
    }
}

async fn setup_test_ui() -> Result<(State, Vec<usize>, Terminal<TestBackend>, jiff::Timestamp)> {
    // Set TZ to UTC for consistent snapshots across environments
    unsafe {
        std::env::set_var("TZ", "UTC");
    }

    let mut client = MockPueueClient::new();
    let state = client.get_state().await?;
    let task_ids: Vec<usize> = state.tasks.keys().cloned().collect();

    let backend = TestBackend::new(80, 24);
    let terminal = Terminal::new(backend)?;

    let now = Local.timestamp_opt(1767225600, 0).unwrap();
    let jiff_now = jiff::Timestamp::from_second(now.timestamp()).unwrap();

    Ok((state, task_ids, terminal, jiff_now))
}

fn buffer_contents(buffer: &Buffer) -> String {
    buffer
        .content
        .chunks(buffer.area.width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn test_ui_snapshot() -> Result<()> {
    let (state, task_ids, mut terminal, jiff_now) = setup_test_ui().await?;
    let mut table_state = TableState::default();
    table_state.select(Some(0));

    terminal.draw(|f| {
        let mut ui_state = ui::UiState {
            state: &Some(state),
            table_state: &mut table_state,
            task_ids: &task_ids,
            now: jiff_now.clone(),
            show_details: false,
            filter_text: "",
            input_mode: false,
            sort_mode: false,
            sort_field: SortField::default(),
            log_view: None,
            connection_error: None,
            error_modal: None,
            selected_task_ids: &HashSet::new(),
            help_mode: false,
            help_scroll_offset: 0,
            custom_commands: &BTreeMap::new(),
            config_path: None,
        };
        ui::draw(f, &mut ui_state);
    })?;

    let ui = buffer_contents(terminal.backend().buffer());

    insta::assert_snapshot!(ui);

    Ok(())
}

#[tokio::test]
async fn test_ui_snapshot_with_details() -> Result<()> {
    let (mut state, task_ids, mut terminal, jiff_now) = setup_test_ui().await?;
    let mut table_state = TableState::default();
    table_state.select(Some(0));

    // Add a long command and path to demonstrate wrapping
    if let Some(task) = state.tasks.get_mut(&0) {
        task.command = "long_command --option1 value1 --option2 value2 --option3 value3 --option4 value4 --option5 value5 --option6 value6".to_string();
        task.path = PathBuf::from(
            "/very/long/path/to/a/directory/that/should/definitely/wrap/at/some/point/in/the/ui/view",
        );
    }

    terminal.draw(|f| {
        let mut ui_state = ui::UiState {
            state: &Some(state),
            table_state: &mut table_state,
            task_ids: &task_ids,
            now: jiff_now,
            show_details: true,
            filter_text: "",
            input_mode: false,
            sort_mode: false,
            sort_field: SortField::default(),
            log_view: None,
            connection_error: None,
            error_modal: None,
            selected_task_ids: &HashSet::new(),
            help_mode: false,
            help_scroll_offset: 0,
            custom_commands: &BTreeMap::new(),
            config_path: None,
        };
        ui::draw(f, &mut ui_state);
    })?;

    let ui = buffer_contents(terminal.backend().buffer());

    insta::assert_snapshot!(ui);

    Ok(())
}

#[tokio::test]
async fn test_ui_snapshot_with_scrollbar() -> Result<()> {
    let mut client = MockPueueClient::new();
    let mut state = client.get_state().await?;

    // Add many tasks to trigger scrollbar
    // Terminal height is 24, table area is ~16, visible rows ~13
    let now = Local.timestamp_opt(1767225600, 0).unwrap();
    for i in 3..20 {
        let task = Task {
            id: i,
            created_at: now,
            original_command: format!("sleep {}", i),
            command: format!("sleep {}", i),
            path: PathBuf::from("/tmp"),
            envs: HashMap::new(),
            group: "default".to_string(),
            dependencies: vec![],
            priority: 0,
            label: None,
            status: TaskStatus::Queued { enqueued_at: now },
        };
        state.tasks.insert(i, task);
    }

    let mut task_ids: Vec<usize> = state.tasks.keys().cloned().collect();
    task_ids.sort();

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend)?;
    let jiff_now = jiff::Timestamp::from_second(now.timestamp()).unwrap();

    let mut table_state = TableState::default();

    // Select the last task to move scrollbar to the bottom
    table_state.select(Some(task_ids.len().saturating_sub(1)));

    terminal.draw(|f| {
        let mut ui_state = ui::UiState {
            state: &Some(state),
            table_state: &mut table_state,
            task_ids: &task_ids,
            now: jiff_now.clone(),
            show_details: false,
            filter_text: "",
            input_mode: false,
            sort_mode: false,
            sort_field: SortField::default(),
            log_view: None,
            connection_error: None,
            error_modal: None,
            selected_task_ids: &HashSet::new(),
            help_mode: false,
            help_scroll_offset: 0,
            custom_commands: &BTreeMap::new(),
            config_path: None,
        };
        ui::draw(f, &mut ui_state);
    })?;

    let ui = buffer_contents(terminal.backend().buffer());

    insta::assert_snapshot!(ui);

    Ok(())
}

#[tokio::test]
async fn test_ui_snapshot_filter_active() -> Result<()> {
    let (state, _, mut terminal, jiff_now) = setup_test_ui().await?;
    let mut table_state = TableState::default();
    table_state.select(Some(0));

    // Filter tasks by "1"
    let task_ids: Vec<usize> = state
        .tasks
        .iter()
        .filter(|(id, task)| ui::format_task(**id, task, &jiff_now).matches_filter("1"))
        .map(|(id, _)| *id)
        .collect();

    // Show filter active state with "1" as text
    terminal.draw(|f| {
        let mut ui_state = ui::UiState {
            state: &Some(state),
            table_state: &mut table_state,
            task_ids: &task_ids,
            now: jiff_now,
            show_details: false,
            filter_text: "1",
            input_mode: false,
            sort_mode: false,
            sort_field: SortField::default(),
            log_view: None,
            connection_error: None,
            error_modal: None,
            selected_task_ids: &HashSet::new(),
            help_mode: false,
            help_scroll_offset: 0,
            custom_commands: &BTreeMap::new(),
            config_path: None,
        };
        ui::draw(f, &mut ui_state);
    })?;

    let ui = buffer_contents(terminal.backend().buffer());

    insta::assert_snapshot!(ui);

    Ok(())
}

#[tokio::test]
async fn test_ui_snapshot_remove_task() -> Result<()> {
    let (mut state, _, mut terminal, jiff_now) = setup_test_ui().await?;
    let mut table_state = TableState::default();
    table_state.select(Some(0));

    // Remove task with ID 1 to verify UI updates
    state.tasks.remove(&1);
    let mut task_ids: Vec<usize> = state.tasks.keys().cloned().collect();
    task_ids.sort();

    terminal.draw(|f| {
        let mut ui_state = ui::UiState {
            state: &Some(state),
            table_state: &mut table_state,
            task_ids: &task_ids,
            now: jiff_now,
            show_details: false,
            filter_text: "",
            input_mode: false,
            sort_mode: false,
            sort_field: SortField::default(),
            log_view: None,
            connection_error: None,
            error_modal: None,
            selected_task_ids: &HashSet::new(),
            help_mode: false,
            help_scroll_offset: 0,
            custom_commands: &BTreeMap::new(),
            config_path: None,
        };
        ui::draw(f, &mut ui_state);
    })?;

    let ui = buffer_contents(terminal.backend().buffer());

    insta::assert_snapshot!(ui);

    Ok(())
}

#[tokio::test]
async fn test_ui_snapshot_log_view() -> Result<()> {
    let (state, task_ids, mut terminal, jiff_now) = setup_test_ui().await?;
    let mut table_state = TableState::default();
    table_state.select(Some(0));

    // Log with tabs and long lines
    let logs = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.\n\
                \t- Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.\n\
                \t- Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur.\n\
                Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.\n\
                Long line: Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur.";
    let scroll_offset = 0;

    terminal.draw(|f| {
        let mut ui_state = ui::UiState {
            state: &Some(state),
            table_state: &mut table_state,
            task_ids: &task_ids,
            now: jiff_now,
            show_details: false,
            filter_text: "",
            input_mode: false,
            sort_mode: false,
            sort_field: SortField::default(),
            log_view: Some((logs, scroll_offset)),
            connection_error: None,
            error_modal: None,
            selected_task_ids: &HashSet::new(),
            help_mode: false,
            help_scroll_offset: 0,
            custom_commands: &BTreeMap::new(),
            config_path: None,
        };
        ui::draw(f, &mut ui_state);
    })?;

    let ui = buffer_contents(terminal.backend().buffer());

    insta::assert_snapshot!(ui);

    Ok(())
}

#[tokio::test]
async fn test_ui_snapshot_log_view_end_key() -> Result<()> {
    // This test is designed to fail if using textwrap of naive line count
    // instead of Ratatui's Paragraph wrapper

    // Width 42 -> content wrap width is 40 (after borders).
    let backend = TestBackend::new(42, 4);
    let mut terminal = Terminal::new(backend)?;

    let mut table_state = TableState::default();
    table_state.select(Some(0));

    let mut log_state = LogState::new(0);
    log_state.logs = [
        // A long line that will be wrapped. At width>=6, Ratatui and textwrap disagree on
        // wrapped row count, so a textwrap-based "End" scroll doesn't reach the end of the logs.
        "PATH  /very/long/path/preceded/with/spaces/to/align/columns",
        // Final marker line we must be able to reach with End/G
        // Keep the marker <= page_width so it doesn't wrap (makes the assertion simple).
        "ZZ",
    ]
    .join("\n");

    // Simulate 'G' / End key press.
    // Borders take 2 lines => 2 lines of content visible and 40 columns of content width for wrapping.
    let page_height = 4 - 2;
    let page_width = 42 - 2;
    log_state.handle_key(
        KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT),
        page_height,
        page_width,
    );
    log_state.update_autoscroll(page_height, page_width);

    terminal.draw(|f| {
        let mut ui_state = ui::UiState {
            state: &None,
            table_state: &mut table_state,
            task_ids: &[],
            now: jiff::Timestamp::now(),
            show_details: false,
            filter_text: "",
            input_mode: false,
            sort_mode: false,
            sort_field: SortField::default(),
            log_view: Some((&log_state.logs, log_state.scroll_offset)),
            connection_error: None,
            error_modal: None,
            selected_task_ids: &HashSet::new(),
            help_mode: false,
            help_scroll_offset: 0,
            custom_commands: &BTreeMap::new(),
            config_path: None,
        };
        ui::draw(f, &mut ui_state);
    })?;

    let ui = buffer_contents(terminal.backend().buffer());

    let buffer_lines: Vec<&str> = ui.lines().collect();
    let last_content_line = buffer_lines
        .iter()
        .rev()
        // Only consider actual paragraph lines (not the top/bottom border).
        .find(|line| line.starts_with('│') && line.ends_with('│'))
        .unwrap_or(&"");
    assert!(
        // Second to last line is the wrapped tail of the long path
        last_content_line.contains("ZZ"),
        "Expected last visible content line to contain the end marker, but got: {:?}",
        last_content_line
    );

    insta::assert_snapshot!(ui);

    Ok(())
}

#[tokio::test]
async fn test_ui_snapshot_log_view_end_key_then_down() -> Result<()> {
    // Ensure "scroll to end" (G/End) followed by a further down-step doesn't move beyond the end.

    // Width 42 -> content wrap width is 40 (after borders).
    let backend = TestBackend::new(42, 4);
    let mut terminal = Terminal::new(backend)?;

    let mut table_state = TableState::default();
    table_state.select(Some(0));

    let mut log_state = LogState::new(0);
    log_state.logs = ["AA", "ZZ"].join("\n");

    // Borders take 2 lines => 2 lines of content visible and 40 columns of content width for wrapping.
    let page_height = 4 - 2;
    let page_width = 42 - 2;

    // 'G' / End
    log_state.handle_key(
        KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT),
        page_height,
        page_width,
    );
    log_state.update_autoscroll(page_height, page_width);

    // One line beyond end
    log_state.handle_key(
        KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
        page_height,
        page_width,
    );
    log_state.update_autoscroll(page_height, page_width);

    terminal.draw(|f| {
        let mut ui_state = ui::UiState {
            state: &None,
            table_state: &mut table_state,
            task_ids: &[],
            now: jiff::Timestamp::now(),
            show_details: false,
            filter_text: "",
            input_mode: false,
            sort_mode: false,
            sort_field: SortField::default(),
            log_view: Some((&log_state.logs, log_state.scroll_offset)),
            connection_error: None,
            error_modal: None,
            selected_task_ids: &HashSet::new(),
            help_mode: false,
            help_scroll_offset: 0,
            custom_commands: &BTreeMap::new(),
            config_path: None,
        };
        ui::draw(f, &mut ui_state);
    })?;

    let ui = buffer_contents(terminal.backend().buffer());

    // Verify clamping: "ZZ" should remain on the last visible content line.
    let buffer_lines: Vec<&str> = ui.lines().collect();
    let content_lines: Vec<&&str> = buffer_lines
        .iter()
        .filter(|line| line.starts_with('│') && line.ends_with('│'))
        .collect();
    assert!(
        content_lines[1].contains("ZZ"),
        "Expected last visible content line to contain the end marker, but got: {:?}",
        content_lines[1]
    );

    insta::assert_snapshot!(ui);

    Ok(())
}

/// Test that selection follows task ID when sort order changes
#[tokio::test]
async fn test_selection_follows_task_id_after_sort() -> Result<()> {
    use crate::App;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    // Create a state with tasks that will reorder when sorted differently
    let mut state = State::default();
    let now = Local.timestamp_opt(1767225600, 0).unwrap();

    // Task 0: command "zzz" (will be last alphabetically)
    let task0 = Task {
        id: 0,
        created_at: now,
        original_command: "zzz".to_string(),
        command: "zzz".to_string(),
        path: PathBuf::from("/tmp"),
        envs: HashMap::new(),
        group: "default".to_string(),
        dependencies: vec![],
        priority: 0,
        label: None,
        status: TaskStatus::Queued { enqueued_at: now },
    };

    // Task 1: command "aaa" (will be first alphabetically)
    let task1 = Task {
        id: 1,
        created_at: now,
        original_command: "aaa".to_string(),
        command: "aaa".to_string(),
        path: PathBuf::from("/tmp"),
        envs: HashMap::new(),
        group: "default".to_string(),
        dependencies: vec![],
        priority: 0,
        label: None,
        status: TaskStatus::Queued { enqueued_at: now },
    };

    state.tasks.insert(0, task0);
    state.tasks.insert(1, task1);

    let mock_client = MockPueueClient {
        state: state.clone(),
    };
    let mut app = App::new(mock_client, Config::default());
    app.state = Some(state);

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend)?;

    // 1. select a task (Task 1 at row 1 when sorted by ID)
    app.table_state.select(Some(1));
    app.update_current_task_id();
    assert_eq!(app.current_task_id, Some(1));

    // 2. resorts the table so that the selected task ends up in a different row
    app.sort_field = SortField::Command;

    // 3. checks that the selected is still for the task in 1.
    // This triggers the sync logic via redraw
    terminal.draw(|f| app.draw(f))?;

    assert_eq!(
        app.current_task_id,
        Some(1),
        "Selection should still be Task 1"
    );
    assert_eq!(
        app.table_state.selected(),
        Some(0),
        "Task 1 (aaa) should now be at row 0"
    );

    let ui = buffer_contents(terminal.backend().buffer());

    insta::assert_snapshot!(ui);

    Ok(())
}

/// Test that deleting the last task keeps selection on the new last row
#[tokio::test]
async fn test_selection_after_deleting_last_task() -> Result<()> {
    use crate::App;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut state = State::default();
    let now = Local.timestamp_opt(1767225600, 0).unwrap();

    for id in 0..3 {
        let task = Task {
            id,
            created_at: now,
            original_command: format!("task_{id}"),
            command: format!("task_{id}"),
            path: PathBuf::from("/tmp"),
            envs: HashMap::new(),
            group: "default".to_string(),
            dependencies: vec![],
            priority: 0,
            label: None,
            status: TaskStatus::Queued { enqueued_at: now },
        };
        state.tasks.insert(id, task);
    }

    let mock_client = MockPueueClient {
        state: state.clone(),
    };
    let mut app = App::new(mock_client, Config::default());
    app.state = Some(state);

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend)?;

    // Select the last row (task 2)
    app.table_state.select(Some(2));
    app.update_current_task_id();
    assert_eq!(app.current_task_id, Some(2));

    // Remove the last task and re-sync selection
    if let Some(app_state) = app.state.as_mut() {
        app_state.tasks.remove(&2);
    }
    app.update_current_task_id();

    assert_eq!(
        app.table_state.selected(),
        Some(1),
        "Selection should move to the new last row"
    );
    assert_eq!(
        app.current_task_id,
        Some(1),
        "Selection should stay on the previous row after deletion"
    );

    // Ensure draw/sync doesn't reset to the first row
    terminal.draw(|f| app.draw(f))?;
    assert_eq!(app.table_state.selected(), Some(1));
    assert_eq!(app.current_task_id, Some(1));

    Ok(())
}

/// Test multi-select mode with 2 of 4 items selected
#[tokio::test]
async fn test_ui_snapshot_multiselect() -> Result<()> {
    // Set TZ to UTC for consistent snapshots across environments
    unsafe {
        std::env::set_var("TZ", "UTC");
    }

    let mut state = State::default();
    let now = Local.timestamp_opt(1767225600, 0).unwrap();

    // Create 4 tasks
    for i in 0..4 {
        let task = Task {
            id: i,
            created_at: now,
            original_command: format!("task_{}", i),
            command: format!("task_{}", i),
            path: PathBuf::from("/tmp"),
            envs: HashMap::new(),
            group: "default".to_string(),
            dependencies: vec![],
            priority: 0,
            label: None,
            status: TaskStatus::Queued { enqueued_at: now },
        };
        state.tasks.insert(i, task);
    }

    let mut task_ids: Vec<usize> = state.tasks.keys().cloned().collect();
    task_ids.sort();

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend)?;

    let jiff_now = jiff::Timestamp::from_second(now.timestamp()).unwrap();
    let mut table_state = TableState::default();
    table_state.select(Some(0));

    // Select tasks 0 and 2 (2 of 4)
    let mut selected_task_ids = HashSet::new();
    selected_task_ids.insert(0);
    selected_task_ids.insert(2);

    terminal.draw(|f| {
        let mut ui_state = ui::UiState {
            state: &Some(state),
            table_state: &mut table_state,
            task_ids: &task_ids,
            now: jiff_now,
            show_details: false,
            filter_text: "",
            input_mode: false,
            sort_mode: false,
            sort_field: SortField::default(),
            log_view: None,
            connection_error: None,
            error_modal: None,
            selected_task_ids: &selected_task_ids,
            help_mode: false,
            help_scroll_offset: 0,
            custom_commands: &BTreeMap::new(),
            config_path: None,
        };
        ui::draw(f, &mut ui_state);
    })?;

    let ui = buffer_contents(terminal.backend().buffer());

    insta::assert_snapshot!(ui);

    Ok(())
}

// Custom command tests
use crate::exec::spawn_process;

/// Test that custom command runs in the specified directory
#[test]
fn test_custom_command_runs_in_correct_directory() {

    let temp_dir = tempfile::tempdir().unwrap();
    let marker_file = temp_dir.path().join("marker.txt");

    // Command that creates a file in the current directory
    let result = spawn_process(
        &["touch".to_string(), "marker.txt".to_string()],
        temp_dir.path(),
    );

    assert!(result.is_ok());
    assert!(
        marker_file.exists(),
        "Command should run in specified directory"
    );
}

/// Test that command arguments are passed correctly
#[test]
fn test_custom_command_passes_arguments() {

    let temp_dir = tempfile::tempdir().unwrap();
    let output_file = temp_dir.path().join("output.txt");

    // Write specific content to verify args were passed
    let result = spawn_process(
        &[
            "sh".to_string(),
            "-c".to_string(),
            format!("echo 'hello world' > {}", output_file.display()),
        ],
        temp_dir.path(),
    );

    assert!(result.is_ok());
    let content = std::fs::read_to_string(&output_file).unwrap();
    assert_eq!(content.trim(), "hello world");
}

/// Test that command failure is reported
#[test]
fn test_custom_command_reports_failure() {
    use std::path::Path;

    let result = spawn_process(
        &["sh".to_string(), "-c".to_string(), "exit 42".to_string()],
        Path::new("/tmp"),
    );

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("42"));
}

/// Test that pwd matches working directory in spawned command
#[test]
fn test_custom_command_pwd_matches_working_directory() {

    let temp_dir = tempfile::tempdir().unwrap();
    let output_file = temp_dir.path().join("pwd.txt");

    let result = spawn_process(
        &[
            "sh".to_string(),
            "-c".to_string(),
            format!("pwd > {}", output_file.display()),
        ],
        temp_dir.path(),
    );

    assert!(result.is_ok());
    let pwd = std::fs::read_to_string(&output_file).unwrap();
    // Canonicalize both to handle symlinks (e.g., /tmp -> /private/tmp on macOS)
    assert_eq!(
        std::fs::canonicalize(pwd.trim()).unwrap(),
        std::fs::canonicalize(temp_dir.path()).unwrap()
    );
}

// Help mode tests

/// Test help modal UI
#[tokio::test]
async fn test_ui_snapshot_help_mode() -> Result<()> {
    // Set TZ to UTC for consistent snapshots across environments
    unsafe {
        std::env::set_var("TZ", "UTC");
    }

    let (state, task_ids, mut terminal, jiff_now) = setup_test_ui().await?;
    let mut table_state = TableState::default();
    table_state.select(Some(0));

    // Create some custom commands for display
    let mut custom_commands = BTreeMap::new();
    custom_commands.insert(
        "lazygit".to_string(),
        crate::config::CustomCommand {
            key: "ctrl+g".to_string(),
            cmd: vec!["lazygit".to_string()],
        },
    );
    custom_commands.insert(
        "editor".to_string(),
        crate::config::CustomCommand {
            key: "alt+e".to_string(),
            cmd: vec!["nvim".to_string()],
        },
    );

    terminal.draw(|f| {
        let mut ui_state = ui::UiState {
            state: &Some(state),
            table_state: &mut table_state,
            task_ids: &task_ids,
            now: jiff_now,
            show_details: false,
            filter_text: "",
            input_mode: false,
            sort_mode: false,
            sort_field: SortField::default(),
            log_view: None,
            connection_error: None,
            error_modal: None,
            selected_task_ids: &HashSet::new(),
            help_mode: true,
            help_scroll_offset: 0,
            custom_commands: &custom_commands,
            config_path: Some(std::path::Path::new("/home/user/.config/pui/config.toml")),
        };
        ui::draw(f, &mut ui_state);
    })?;

    let ui = buffer_contents(terminal.backend().buffer());

    insta::assert_snapshot!(ui);

    Ok(())
}

/// Test help modal UI scrolled to the bottom
#[tokio::test]
async fn test_ui_snapshot_help_mode_scrolled_bottom() -> Result<()> {
    // Set TZ to UTC for consistent snapshots across environments
    unsafe {
        std::env::set_var("TZ", "UTC");
    }

    let (state, task_ids, mut terminal, jiff_now) = setup_test_ui().await?;
    let mut table_state = TableState::default();
    table_state.select(Some(0));

    // Create some custom commands for display
    let mut custom_commands = BTreeMap::new();
    custom_commands.insert(
        "lazygit".to_string(),
        crate::config::CustomCommand {
            key: "ctrl+g".to_string(),
            cmd: vec!["lazygit".to_string()],
        },
    );
    custom_commands.insert(
        "editor".to_string(),
        crate::config::CustomCommand {
            key: "alt+e".to_string(),
            cmd: vec!["nvim".to_string()],
        },
    );

    let modal_area = ui::centered_rect(70, 80, Rect::new(0, 0, 80, 24));
    let content_height = modal_area.height.saturating_sub(2);
    let content_width = modal_area.width.saturating_sub(2).max(1);
    let line_count = ui::help_modal_line_count(
        &custom_commands,
        Some(std::path::Path::new("/home/user/.config/pui/config.toml")),
        content_width,
        content_height,
    );
    let max_offset = line_count.saturating_sub(content_height);

    terminal.draw(|f| {
        let mut ui_state = ui::UiState {
            state: &Some(state),
            table_state: &mut table_state,
            task_ids: &task_ids,
            now: jiff_now,
            show_details: false,
            filter_text: "",
            input_mode: false,
            sort_mode: false,
            sort_field: SortField::default(),
            log_view: None,
            connection_error: None,
            error_modal: None,
            selected_task_ids: &HashSet::new(),
            help_mode: true,
            help_scroll_offset: max_offset,
            custom_commands: &custom_commands,
            config_path: Some(std::path::Path::new("/home/user/.config/pui/config.toml")),
        };
        ui::draw(f, &mut ui_state);
    })?;

    let ui = buffer_contents(terminal.backend().buffer());

    insta::assert_snapshot!(ui);

    Ok(())
}

/// Test find_matching_custom_command with simple key
#[tokio::test]
async fn test_find_matching_custom_command_simple_key() {
    use crate::App;
    use crate::config::{Config, CustomCommand};

    let mut config = Config::default();
    config.custom_commands.insert(
        "test".to_string(),
        CustomCommand {
            key: "g".to_string(),
            cmd: vec!["echo".to_string(), "test".to_string()],
        },
    );

    let mock_client = MockPueueClient::new();
    let app = App::new(mock_client, config);

    // Test matching key
    let key_event = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
    let result = app.find_matching_custom_command(&key_event);
    assert!(result.is_some());
    let (name, _) = result.unwrap();
    assert_eq!(name, "test");

    // Test non-matching key
    let key_event = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE);
    let result = app.find_matching_custom_command(&key_event);
    assert!(result.is_none());

    // Test with modifier when none expected - should not match
    let key_event = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL);
    let result = app.find_matching_custom_command(&key_event);
    assert!(result.is_none());
}

/// Test find_matching_custom_command with ctrl modifier
#[tokio::test]
async fn test_find_matching_custom_command_ctrl_key() {
    use crate::App;
    use crate::config::{Config, CustomCommand};

    let mut config = Config::default();
    config.custom_commands.insert(
        "test".to_string(),
        CustomCommand {
            key: "ctrl+g".to_string(),
            cmd: vec!["echo".to_string()],
        },
    );

    let mock_client = MockPueueClient::new();
    let app = App::new(mock_client, config);

    // Test matching key with ctrl
    let key_event = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL);
    let result = app.find_matching_custom_command(&key_event);
    assert!(result.is_some());

    // Test without modifier - should not match
    let key_event = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
    let result = app.find_matching_custom_command(&key_event);
    assert!(result.is_none());
}

/// Test find_matching_custom_command with alt modifier
#[tokio::test]
async fn test_find_matching_custom_command_alt_key() {
    use crate::App;
    use crate::config::{Config, CustomCommand};

    let mut config = Config::default();
    config.custom_commands.insert(
        "test".to_string(),
        CustomCommand {
            key: "alt+r".to_string(),
            cmd: vec!["echo".to_string()],
        },
    );

    let mock_client = MockPueueClient::new();
    let app = App::new(mock_client, config);

    // Test matching key with alt
    let key_event = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::ALT);
    let result = app.find_matching_custom_command(&key_event);
    assert!(result.is_some());
}

/// Test find_matching_custom_command with opt modifier (alias for alt)
#[tokio::test]
async fn test_find_matching_custom_command_opt_key() {
    use crate::App;
    use crate::config::{Config, CustomCommand};

    let mut config = Config::default();
    config.custom_commands.insert(
        "test".to_string(),
        CustomCommand {
            key: "opt+q".to_string(),
            cmd: vec!["echo".to_string()],
        },
    );

    let mock_client = MockPueueClient::new();
    let app = App::new(mock_client, config);

    // Test matching key with alt (opt is alias for alt)
    let key_event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::ALT);
    let result = app.find_matching_custom_command(&key_event);
    assert!(result.is_some());
}

/// Test config command parsing with new single key format
#[test]
fn test_config_parse_with_modifiers() {
    let toml = r#"
[custom_commands]
lazygit = { key = "ctrl+g", cmd = ["lazygit"] }
editor = { key = "alt+e", cmd = ["nvim", "."] }
simple = { key = "t", cmd = ["echo", "test"] }
"#;
    let config: Config = toml::from_str(toml).unwrap();
    assert_eq!(config.custom_commands.len(), 3);

    let lazygit = config.custom_commands.get("lazygit").unwrap();
    assert_eq!(lazygit.key, "ctrl+g");

    let editor = config.custom_commands.get("editor").unwrap();
    assert_eq!(editor.key, "alt+e");

    let simple = config.custom_commands.get("simple").unwrap();
    assert_eq!(simple.key, "t");
}
