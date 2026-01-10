use anyhow::Result;
use chrono::{Local, TimeZone};
use pueue_lib::state::State;
use pueue_lib::task::{Task, TaskResult, TaskStatus};
use ratatui::{backend::TestBackend, widgets::TableState, Terminal};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::pueue_client::PueueClientOps;
use crate::ui;

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

    async fn pause_tasks(&mut self, _ids: Vec<usize>) -> Result<()> {
        Ok(())
    }

    async fn kill_tasks(&mut self, _ids: Vec<usize>) -> Result<()> {
        Ok(())
    }

    async fn remove_tasks(&mut self, _ids: Vec<usize>) -> Result<()> {
        Ok(())
    }

    async fn get_task_log(&mut self, _id: usize) -> Result<Option<String>> {
        Ok(Some("Log line 1\nLog line 2\nLog line 3".to_string()))
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
            log_view: None,
        };
        ui::draw(f, &mut ui_state);
    })?;

    let buffer = terminal.backend().buffer();
    let buffer_string = buffer
        .content
        .chunks(buffer.area.width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!(buffer_string);

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
        task.path = PathBuf::from("/very/long/path/to/a/directory/that/should/definitely/wrap/at/some/point/in/the/ui/view");
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
            log_view: None,
        };
        ui::draw(f, &mut ui_state);
    })?;

    let buffer = terminal.backend().buffer();
    let buffer_string = buffer
        .content
        .chunks(buffer.area.width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!(buffer_string);

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
            status: TaskStatus::Queued {
                enqueued_at: now,
            },
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
            log_view: None,
        };
        ui::draw(f, &mut ui_state);
    })?;

    let buffer = terminal.backend().buffer();
    let buffer_string = buffer
        .content
        .chunks(buffer.area.width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!(buffer_string);

    Ok(())
}

#[tokio::test]
async fn test_ui_snapshot_filter_active() -> Result<()> {
    let (state, _, mut terminal, jiff_now) = setup_test_ui().await?;
    let mut table_state = TableState::default();
    table_state.select(Some(0));

    // Filter tasks by "1"
    let task_ids: Vec<usize> = state.tasks.iter()
        .filter(|(id, task)| {
            ui::format_task(**id, task, &jiff_now).matches_filter("1")
        })
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
            log_view: None,
        };
        ui::draw(f, &mut ui_state);
    })?;

    let buffer = terminal.backend().buffer();
    let buffer_string = buffer
        .content
        .chunks(buffer.area.width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!(buffer_string);

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
            log_view: Some((logs, scroll_offset)),
        };
        ui::draw(
            f,
            &mut ui_state
        );
    })?;

    let buffer = terminal.backend().buffer();
    let buffer_string = buffer
        .content
        .chunks(buffer.area.width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!(buffer_string);

    Ok(())
}
