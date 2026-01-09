use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table, TableState, Paragraph},
    Frame,
};
use std::path::Path;
use pueue_lib::state::State;
use pueue_lib::task::TaskStatus;

fn status_name(status: &TaskStatus) -> &str {
    match status {
        TaskStatus::Locked { .. } => "Locked",
        TaskStatus::Stashed { .. } => "Stashed",
        TaskStatus::Queued { .. } => "Queued",
        TaskStatus::Running { .. } => "Running",
        TaskStatus::Paused { .. } => "Paused",
        TaskStatus::Done { .. } => "Done",
    }
}

pub fn draw(f: &mut Frame, state: &Option<State>, table_state: &mut TableState, task_ids: &[usize], now: jiff::Timestamp) {
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
            let status = status_name(&task.status);
            let style = match status {
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
                    now.clone()
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

        f.render_stateful_widget(task_table, main_chunks[0], table_state);

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
                        now.clone()
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
                    "ID: {}\nStatus: {}\nCommand: {}\nPath: {}\nDuration: {}\nGroup: {}\n",
                    id, status_name(&task.status), command_basename, shortened_path, duration_str, task.group
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
}
