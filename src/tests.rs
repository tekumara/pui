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

        // Fixed time for snapshot stability: 2024-01-01 00:00:00 (Local)
        let now = Local.timestamp_opt(1704067200, 0).unwrap();

        let task1 = Task::new(
            "sleep 60".to_string(),
            PathBuf::from("/tmp"),
            HashMap::new(),
            "default".to_string(),
            TaskStatus::Running {
                enqueued_at: now,
                start: now,
            },
            vec![],
            0,
            None
        );
        // Task::new sets id to 0 by default probably, or we need to set it?
        // Task::new doesn't take ID?
        // Checking pui code: `s.tasks` is BTreeMap<usize, Task>.
        // Task has `id` field. `Task::new` probably generates one or takes it?
        // The error message for `new` showed:
        // original_command, path, envs, group, status, dependencies, label.
        // It didn't show `id`. So `Task::new` might not take `id`.
        // We might need to set `task.id = 0;` manually after construction if field is pub.

        let mut task1 = task1;
        task1.id = 0;

        let task2 = Task::new(
            "echo 'hello'".to_string(),
            PathBuf::from("/home/user"),
            HashMap::new(),
            "default".to_string(),
            TaskStatus::Done {
                enqueued_at: now,
                start: now,
                end: now,
                result: TaskResult::Success,
            },
            vec![],
            0,
            None
        );
        let mut task2 = task2;
        task2.id = 1;

        state.tasks.insert(0, task1);
        state.tasks.insert(1, task2);

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

#[tokio::test]
async fn test_ui_snapshot() -> Result<()> {
    // Set TZ to UTC for consistent snapshots across environments
    unsafe {
        std::env::set_var("TZ", "UTC");
    }

    let mut client = MockPueueClient::new();
    let state = client.get_state().await?;
    let task_ids: Vec<usize> = state.tasks.keys().cloned().collect();

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend)?;
    let mut table_state = TableState::default();
    table_state.select(Some(0));

    let now = Local.timestamp_opt(1704067200, 0).unwrap();
    let jiff_now = jiff::Timestamp::from_second(now.timestamp()).unwrap();

    terminal.draw(|f| {
        ui::draw(f, &Some(state), &mut table_state, &task_ids, jiff_now);
    })?;

    insta::assert_debug_snapshot!(terminal.backend());

    Ok(())
}
