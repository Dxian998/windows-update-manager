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

pub fn render_status_block(is_blocked: bool) -> Paragraph<'static> {
    let theme_color = if is_blocked { Color::Red } else { Color::Green };

    let status_text = if is_blocked {
        Span::styled(
            "UPDATES ARE BLOCKED!",
            Style::default()
                .fg(theme_color)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            "UPDATES ARE ENABLED!",
            Style::default()
                .fg(theme_color)
                .add_modifier(Modifier::BOLD),
        )
    };

    let registry_status = super::update::get_registry_status();
    let wua_status = super::services::get_service_status("wuauserv");
    let uso_status = super::services::get_service_status("UsoSvc");
    let medic_status = super::services::get_service_status("WaaSMedicSvc");

    let status_lines = vec![
        Spans::from(status_text),
        Spans::from(""),
        Spans::from(vec![
            Span::raw("Registry Status:   "),
            Span::styled(registry_status, Style::default().fg(theme_color)),
        ]),
        Spans::from(vec![
            Span::raw("wuauserv:         "),
            Span::styled(wua_status, Style::default().fg(theme_color)),
        ]),
        Spans::from(vec![
            Span::raw("UsoSvc:           "),
            Span::styled(uso_status, Style::default().fg(theme_color)),
        ]),
        Spans::from(vec![
            Span::raw("WaaSMedicSvc:     "),
            Span::styled(medic_status, Style::default().fg(theme_color)),
        ]),
    ];

    Paragraph::new(status_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::raw("Update Status"))
                .border_style(Style::default().fg(theme_color)),
        )
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left)
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
            Constraint::Length(8),
            Constraint::Min(9),
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

    // Status block
    let status_paragraph = render_status_block(app.update_blocked);
    frame.render_widget(status_paragraph, chunks[1]);

    // Menu items
    let menu_items = if app.update_blocked {
        vec!["Enable Windows Updates", "Check the source code"]
    } else {
        vec!["Disable Windows Updates", "Check the source code"]
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
        let overlay_area = centered_rect(30, 7, size);
        let block = Block::default()
            .title(Span::styled("Working...", Style::default().fg(theme_color)))
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::Black))
            .border_style(Style::default().fg(theme_color));

        let para = Paragraph::new(Span::styled(
            "Please wait while the operation complees...",
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
        return false;
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
            let item_count = 2;
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
                    1 => app.open_github(),
                    _ => (),
                }
            }
            true
        }
        KeyCode::Esc | KeyCode::Char('q') => false,
        _ => true,
    }
}
