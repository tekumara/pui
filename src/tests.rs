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
        ui::draw(f, &Some(state), &mut table_state, &task_ids, jiff_now, false, "", false);
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
    let (state, task_ids, mut terminal, jiff_now) = setup_test_ui().await?;
    let mut table_state = TableState::default();
    table_state.select(Some(0));

    terminal.draw(|f| {
        ui::draw(f, &Some(state), &mut table_state, &task_ids, jiff_now, true, "", false);
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
        ui::draw(f, &Some(state), &mut table_state, &task_ids, jiff_now, false, "", false);
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
        ui::draw(f, &Some(state), &mut table_state, &task_ids, jiff_now, false, "1", false);
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
