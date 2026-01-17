use crate::SortField;
use pueue_lib::state::State;
use pueue_lib::task::{Task, TaskResult, TaskStatus};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table, TableState, Wrap,
    },
};
use std::path::Path;

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
        let end_ts = end
            .map(|e| jiff::Timestamp::from_second(e.timestamp()).unwrap())
            .unwrap_or_else(|| now.clone());
        let duration = end_ts.duration_since(start_ts);

        if duration.as_secs() < 60 {
            format!("{}s", duration.as_secs())
        } else if duration.as_secs() < 3600 {
            format!("{}m {}s", duration.as_secs() / 60, duration.as_secs() % 60)
        } else {
            format!(
                "{}h {}m",
                duration.as_secs() / 3600,
                (duration.as_secs() % 3600) / 60
            )
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

pub struct UiState<'a> {
    pub state: &'a Option<State>,
    pub table_state: &'a mut TableState,
    pub task_ids: &'a [usize],
    pub now: jiff::Timestamp,
    pub show_details: bool,
    pub filter_text: &'a str,
    pub input_mode: bool,
    pub sort_mode: bool,
    pub sort_field: SortField,
    pub log_view: Option<(&'a str, u16)>,
    pub connection_error: Option<&'a str>,
    pub error_modal: Option<&'a str>,
}

pub fn draw(f: &mut Frame, ui_state: &mut UiState) {
    if let Some((logs, scroll_offset)) = ui_state.log_view {
        let size = f.area();
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Task Log (Esc to close) ");

        // default tab stop width in terminals is typically 8 characters
        let logs = logs.replace('\t', "        ");
        let p = Paragraph::new(logs)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((scroll_offset, 0)); // (y, x)

        f.render_widget(p, size);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
            ]
            .as_ref(),
        )
        .split(f.area());

    let title_block = Block::default()
        .borders(Borders::ALL)
        .title(" Pui - Pueue TUI ");
    let title = Paragraph::new("j/k/PgUp/PgDn/Home/End: Nav | f: Filter | s: Sort | r: Run | p: Pause | x: Kill | Backspace: Remove | d: Details | q: Quit")
        .block(title_block);
    f.render_widget(title, chunks[0]);

    // Use full width for the table (chunks[1])
    let table_area = chunks[1];

    if let Some(s) = &ui_state.state {
        let rows: Vec<Row> = ui_state
            .task_ids
            .iter()
            .filter_map(|id| s.tasks.get(id).map(|task| (*id, task)))
            .map(|(id, task)| {
                let ft = format_task(id, task, &ui_state.now);
                let style = if ft.status == "Running" || ft.status == "Success" {
                    Style::default().fg(Color::Green)
                } else if ft.status.starts_with("Failed")
                    || ft.status == "Errored"
                    || ft.status == "Killed"
                {
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
                ])
                .style(style)
            })
            .collect();

        let header = Row::new(vec!["Id", "Status", "Command", "Path", "Duration"]).style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Cyan),
        );

        let task_table = Table::new(
            rows,
            [
                Constraint::Length(4),
                Constraint::Length(12),
                Constraint::Percentage(30),
                Constraint::Percentage(30),
                Constraint::Length(10),
            ],
        )
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(" Tasks "))
        .row_highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .bg(Color::Rgb(50, 50, 50)),
        )
        .highlight_symbol(">> ");

        f.render_stateful_widget(task_table, table_area, ui_state.table_state);

        // Calculate visible rows: height - 2 (top/bottom borders) - 1 (header)
        let visible_rows = table_area.height.saturating_sub(3) as usize;
        if ui_state.task_ids.len() > visible_rows {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"));
            let mut scrollbar_state = ScrollbarState::new(ui_state.task_ids.len())
                .viewport_content_length(visible_rows)
                .position(ui_state.table_state.selected().unwrap_or(0));

            f.render_stateful_widget(scrollbar, table_area, &mut scrollbar_state);
        }

        // Task Details Popup
        if ui_state.show_details {
            let selected_id = ui_state
                .table_state
                .selected()
                .and_then(|i| ui_state.task_ids.get(i));

            let details_text = if let Some(id) = selected_id {
                if let Some(task) = s.tasks.get(id) {
                    let ft = format_task(*id, task, &ui_state.now);

                    let mut details = format!(
                        "ID: {}\nStatus: {}\nCommand: {}\nPath: {}\nDuration: {}\nGroup: {}\n",
                        ft.id, ft.status, ft.command, ft.path, ft.duration, ft.group
                    );
                    if let Some(label) = ft.label {
                        details.push_str(&format!("Label: {}\n", label));
                    }
                    details.push_str(&format!(
                        "\nFull Command: {}\nFull Path: {}\n",
                        ft.full_command, ft.full_path
                    ));
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
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Details (Esc to close) "),
                )
                .wrap(Wrap { trim: false });
            f.render_widget(details_block, area);
        }
    } else {
        let loading = Paragraph::new("Loading state from Pueue...")
            .block(Block::default().borders(Borders::ALL).title(" Tasks "));
        f.render_widget(loading, table_area);
    }

    let footer_content: Line = if let Some(error) = ui_state.connection_error {
        Line::from(error).style(Style::default().fg(Color::Red))
    } else if ui_state.sort_mode {
        // Build sort options with highlighting for currently selected field
        let highlight = Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);
        let normal = Style::default();

        let id_style = if ui_state.sort_field == SortField::Id {
            highlight
        } else {
            normal
        };
        let status_style = if ui_state.sort_field == SortField::Status {
            highlight
        } else {
            normal
        };
        let command_style = if ui_state.sort_field == SortField::Command {
            highlight
        } else {
            normal
        };
        let path_style = if ui_state.sort_field == SortField::Path {
            highlight
        } else {
            normal
        };

        Line::from(vec![
            Span::raw("Sort by: "),
            Span::raw("[").style(id_style),
            Span::raw("i").style(id_style.add_modifier(Modifier::UNDERLINED)),
            Span::raw("]d").style(id_style),
            Span::raw(" | "),
            Span::raw("[").style(status_style),
            Span::raw("s").style(status_style.add_modifier(Modifier::UNDERLINED)),
            Span::raw("]tatus").style(status_style),
            Span::raw(" | "),
            Span::raw("[").style(command_style),
            Span::raw("c").style(command_style.add_modifier(Modifier::UNDERLINED)),
            Span::raw("]ommand").style(command_style),
            Span::raw(" | "),
            Span::raw("[").style(path_style),
            Span::raw("p").style(path_style.add_modifier(Modifier::UNDERLINED)),
            Span::raw("]ath").style(path_style),
            Span::raw(" | Esc: cancel"),
        ])
    } else if ui_state.input_mode {
        Line::from(format!("Filter: {}_ (Esc to clear)", ui_state.filter_text))
    } else if !ui_state.filter_text.is_empty() {
        Line::from(format!("Filter: {} (Esc to clear)", ui_state.filter_text))
    } else {
        Line::from("Connected to Pueue daemon")
    };

    let footer = Paragraph::new(footer_content).block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);

    // Error modal (takes priority over everything else)
    if let Some(error) = ui_state.error_modal {
        let area = centered_rect(60, 20, f.area());
        f.render_widget(Clear, area);

        let error_block = Paragraph::new(error)
            .style(Style::default().fg(Color::Red))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Error (Esc to dismiss) ")
                    .border_style(Style::default().fg(Color::Red)),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(error_block, area);
    }
}
