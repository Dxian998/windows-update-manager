use crossterm::event::{KeyCode, KeyEvent};
use tui::{
    Frame,
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let vertical_chunks = Layout::default()
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
        .split(vertical_chunks[1])[1]
}

pub fn render_status_block() -> Paragraph<'static> {
    let (is_blocked, status_details) = super::update::get_update_status();
    let theme_color = if is_blocked { Color::Red } else { Color::Green };

    let header_text = if is_blocked {
        "UPDATES ARE BLOCKED!"
    } else {
        "UPDATES ARE ENABLED!"
    };

    let mut lines: Vec<Spans> = vec![
        Spans::from(Span::styled(
            header_text,
            Style::default()
                .fg(theme_color)
                .add_modifier(Modifier::BOLD),
        )),
        Spans::from(""),
    ];

    for (label, status) in &status_details {
        let row_color = status_color(is_blocked, status);
        let padded_label = format!("{:<26}", label);
        lines.push(Spans::from(vec![
            Span::raw(padded_label),
            Span::styled(status.clone(), Style::default().fg(row_color)),
        ]));
    }

    Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::raw("Update Status"))
                .border_style(Style::default().fg(theme_color)),
        )
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left)
}

fn status_color(overall_blocked: bool, status: &str) -> Color {
    let is_blocked_value = matches!(
        status,
        s if s.starts_with("Disabled")
            || s == "Locked"
            || s == "Active"
            || s == "Blocked"
    );

    if overall_blocked {
        if is_blocked_value {
            Color::Red
        } else {
            Color::Yellow
        }
    } else if is_blocked_value {
        Color::Yellow
    } else {
        Color::Green
    }
}

pub fn render<B: Backend>(frame: &mut Frame<B>, app: &mut super::app::App) {
    let theme_color = if app.update_blocked {
        Color::Red
    } else {
        Color::Green
    };

    let size = frame.size();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(13),
            Constraint::Min(6),
            Constraint::Length(2),
        ])
        .split(size);

    let title = Paragraph::new("Windows Update Manager")
        .style(
            Style::default()
                .fg(theme_color)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center);
    frame.render_widget(title, chunks[0]);

    let status_paragraph = render_status_block();
    frame.render_widget(status_paragraph, chunks[1]);

    let locked_now = super::security::is_registry_key_locked("wuauserv");
    let protect_checkbox = if app.protect_service_settings { "[X]" } else { "[ ]" };
    let protect_label = if app.update_blocked {
        if locked_now {
            format!("{} Protect Service Settings (Locked)", protect_checkbox)
        } else {
            format!("{} Protect Service Settings (Unlocked)", protect_checkbox)
        }
    } else {
        format!("{} Protect Service Settings", protect_checkbox)
    };

    let bits_start = super::services::get_service_start_value("BITS");
    let bits_status_str = match bits_start {
        4 => "Disabled",
        3 => "Manual",
        2 => "Auto",
        _ => "Unknown",
    };
    let bits_menu_item = format!("Toggle BITS Service (Current: {})", bits_status_str);

    let menu_items = if app.update_blocked {
        vec![
            "Enable Windows Updates".to_string(),
            protect_label,
            bits_menu_item,
            "Check the source code".to_string(),
        ]
    } else {
        vec![
            "Disable Windows Updates".to_string(),
            protect_label,
            bits_menu_item,
            "Check the source code".to_string(),
        ]
    };

    let items: Vec<ListItem> = menu_items
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let content = if Some(i) == app.menu_state.selected() {
                Spans::from(Span::styled(
                    format!("> {}", m),
                    Style::default()
                        .fg(Color::White)
                        .bg(theme_color)
                        .add_modifier(Modifier::BOLD),
                ))
            } else {
                Spans::from(Span::styled(
                    format!("  {}", m),
                    Style::default().fg(Color::White),
                ))
            };
            ListItem::new(content)
        })
        .collect();

    let menu = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::raw("Actions"))
                .border_style(Style::default().fg(theme_color)),
        )
        .highlight_style(Style::default().bg(theme_color).fg(Color::White));
    frame.render_stateful_widget(menu, chunks[2], &mut app.menu_state);

    let footer = Paragraph::new("Navigate: ↑ ↓  Select: Enter  Quit: Esc / q")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[3]);

    let busy = *app.busy.lock().unwrap();
    if busy {
        let overlay_area = centered_rect(40, 7, size);
        let block = Block::default()
            .title(Span::styled("Working...", Style::default().fg(theme_color)))
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::Black))
            .border_style(Style::default().fg(theme_color));

        let para = Paragraph::new(Span::styled(
            "Please wait while the operation completes...",
            Style::default().fg(theme_color),
        ))
        .block(block)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

        frame.render_widget(Clear, overlay_area);
        frame.render_widget(para, overlay_area);
    }
}

pub fn handle_key_event(key: KeyEvent, app: &mut super::app::App) -> bool {
    let busy = *app.busy.lock().unwrap();
    if busy {
        return true;
    }

    match key.code {
        KeyCode::Up => {
            let selected = app.menu_state.selected().unwrap_or(0);
            let new_index = selected.saturating_sub(1);
            if new_index != selected {
                app.menu_state.select(Some(new_index));
            }
            true
        }
        KeyCode::Down => {
            let item_count = 4;
            let selected = app.menu_state.selected().unwrap_or(0);
            let new_index = (selected + 1) % item_count;
            if new_index != selected {
                app.menu_state.select(Some(new_index));
            }
            true
        }
        KeyCode::Enter => {
            if let Some(selected) = app.menu_state.selected() {
                match selected {
                    0 => app.toggle_updates(),
                    1 => app.toggle_protect_settings(),
                    2 => app.toggle_bits(),
                    3 => app.open_github(),
                    _ => (),
                }
            }
            true
        }
        KeyCode::Esc | KeyCode::Char('q') => false,
        _ => true,
    }
}
