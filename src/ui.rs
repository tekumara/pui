use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState},
    Frame,
};
use std::path::Path;
use pueue_lib::state::State;
use pueue_lib::task::{Task, TaskResult, TaskStatus};

pub fn status_display(status: &TaskStatus) -> String {
    match status {
        TaskStatus::Locked { .. } => "Locked".to_string(),
        TaskStatus::Stashed { .. } => "Stashed".to_string(),
        TaskStatus::Queued { .. } => "Queued".to_string(),
        TaskStatus::Running { .. } => "Running".to_string(),
        TaskStatus::Paused { .. } => "Paused".to_string(),
        TaskStatus::Done { result, .. } => match result {
            TaskResult::Success => "Success".to_string(),
            TaskResult::Failed(code) => format!("Failed ({})", code),
            TaskResult::Killed => "Killed".to_string(),
            TaskResult::Errored => "Errored".to_string(),
            TaskResult::DependencyFailed => "Dependency Failed".to_string(),
            _ => "Done".to_string(),
        },
    }
}

pub struct FormattedTask<'a> {
    pub id: String,
    pub status: String,
    pub command: String,
    pub path: String,
    pub duration: String,
    pub full_command: &'a str,
    pub full_path: String,
    pub group: &'a str,
    pub label: Option<&'a str>,
}

impl<'a> FormattedTask<'a> {
    pub fn matches_filter(&self, filter: &str) -> bool {
        if filter.is_empty() {
            return true;
        }
        let filter = filter.to_lowercase();
        self.id.to_lowercase().contains(&filter)
            || self.status.to_lowercase().contains(&filter)
            || self.command.to_lowercase().contains(&filter)
            || self.path.to_lowercase().contains(&filter)
    }
}

pub fn format_task<'a>(id: usize, task: &'a Task, now: &jiff::Timestamp) -> FormattedTask<'a> {
    let (start, end) = task.start_and_end();
    let duration_str = if let Some(start) = start {
        let start_ts = jiff::Timestamp::from_second(start.timestamp()).unwrap();
        let end_ts = end.map(|e| jiff::Timestamp::from_second(e.timestamp()).unwrap()).unwrap_or_else(|| now.clone());
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
        .unwrap_or(&task.command)
        .to_string();

    FormattedTask {
        id: id.to_string(),
        status: status_display(&task.status),
        command: command_basename,
        path: tico::tico(&task.path.to_string_lossy()),
        duration: duration_str,
        full_command: &task.command,
        full_path: task.path.to_string_lossy().into_owned(),
        group: &task.group,
        label: task.label.as_deref(),
    }
}

/// helper function to create a centered rect using up certain percentage of the available rect `r`
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub fn draw(f: &mut Frame, state: &Option<State>, table_state: &mut TableState, task_ids: &[usize], now: jiff::Timestamp, show_details: bool, filter_text: &str, input_mode: bool) {
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
    let title = Paragraph::new("j/k: Nav | f: Filter | s: Start | p: Pause | x: Kill | Backspace: Remove | d: Details | q: Quit")
        .block(title_block);
    f.render_widget(title, chunks[0]);

    // Use full width for the table (chunks[1])
    let table_area = chunks[1];

    if let Some(s) = &state {
        let rows: Vec<Row> = task_ids.iter().filter_map(|id| {
            s.tasks.get(id).map(|task| (*id, task))
        }).map(|(id, task)| {
            let ft = format_task(id, task, &now);
            let style = if ft.status == "Running" || ft.status == "Success" {
                Style::default().fg(Color::Green)
            } else if ft.status.starts_with("Failed") || ft.status == "Errored" || ft.status == "Killed" {
                Style::default().fg(Color::Red)
            } else if ft.status == "Queued" {
                Style::default().fg(Color::Yellow)
            } else if ft.status == "Paused" {
                Style::default().fg(Color::Blue)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            Row::new(vec![
                Cell::from(ft.id),
                Cell::from(ft.status),
                Cell::from(ft.command),
                Cell::from(ft.path),
                Cell::from(ft.duration),
            ]).style(style)
        }).collect();

        let header = Row::new(vec!["Id", "Status", "Command", "Path", "Duration"])
            .style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan));

        let task_table = Table::new(rows, [
            Constraint::Length(4),
            Constraint::Length(12),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
            Constraint::Length(10),
        ])
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(" Tasks "))
        .row_highlight_style(Style::default().add_modifier(Modifier::BOLD).bg(Color::Rgb(50, 50, 50)))
        .highlight_symbol(">> ");

        f.render_stateful_widget(task_table, table_area, table_state);

        // Task Details Popup
        if show_details {
            let selected_id = table_state.selected()
                .and_then(|i| task_ids.get(i));

            let details_text = if let Some(id) = selected_id {
                if let Some(task) = s.tasks.get(id) {
                    let ft = format_task(*id, task, &now);

                    let mut details = format!(
                        "ID: {}\nStatus: {}\nCommand: {}\nPath: {}\nDuration: {}\nGroup: {}\n",
                        ft.id, ft.status, ft.command, ft.path, ft.duration, ft.group
                    );
                    if let Some(label) = ft.label {
                        details.push_str(&format!("Label: {}\n", label));
                    }
                    details.push_str(&format!("\nFull Command: {}\nFull Path: {}\n", ft.full_command, ft.full_path));
                    details
                } else {
                    "Task not found".to_string()
                }
            } else {
                "No task selected".to_string()
            };

            let area = centered_rect(60, 60, f.area());
            f.render_widget(Clear, area); // Clear the background

            let details_block = Paragraph::new(details_text)
                .block(Block::default().borders(Borders::ALL).title(" Details (Esc to close) "));
            f.render_widget(details_block, area);
        }

    } else {
        let loading = Paragraph::new("Loading state from Pueue...")
            .block(Block::default().borders(Borders::ALL).title(" Tasks "));
        f.render_widget(loading, table_area);
    }

    let footer_text = if input_mode {
        format!("Filter: {}_ (Esc to clear)", filter_text)
    } else if !filter_text.is_empty() {
        format!("Filter: {} (Esc to clear)", filter_text)
    } else {
        "Connected to Pueue daemon".to_string()
    };

    let footer = Paragraph::new(footer_text)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}
