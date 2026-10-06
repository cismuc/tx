use crate::state::{AppState, ServiceStatus};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use std::io::{self, stdout, Stdout};
use tui_term::widget::PseudoTerminal;

pub struct TerminalGuard;

impl TerminalGuard {
    pub fn init() -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
        enable_raw_mode()?;
        let mut out = stdout();
        execute!(out, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(out);
        Terminal::new(backend)
    }

    pub fn restore() {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture);
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        Self::restore();
    }
}

pub fn install_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        TerminalGuard::restore();
        original_hook(info);
    }));
}

#[derive(Debug, Clone, Copy)]
pub struct LayoutAreas {
    pub sidebar: Rect,
    pub terminal: Rect,
    pub status_bar: Rect,
}

pub fn render_ui(frame: &mut Frame, app: &AppState) -> LayoutAreas {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(1)])
        .split(frame.area());

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(28), Constraint::Min(20)])
        .split(chunks[0]);

    let sidebar_area = main_chunks[0];
    let terminal_area = main_chunks[1];
    let status_area = chunks[1];

    render_sidebar(frame, app, sidebar_area);
    render_terminal(frame, app, terminal_area);
    render_status_bar(frame, app, status_area);

    LayoutAreas {
        sidebar: sidebar_area,
        terminal: terminal_area,
        status_bar: status_area,
    }
}

fn render_sidebar(frame: &mut Frame, app: &AppState, area: Rect) {
    let items: Vec<ListItem> = app
        .services
        .iter()
        .enumerate()
        .map(|(idx, service)| {
            let (status_symbol, status_color) = match &service.status {
                ServiceStatus::Running => ("● ", Color::Green),
                ServiceStatus::Stopped => ("○ ", Color::DarkGray),
                ServiceStatus::Failed(_) => ("■ ", Color::Red),
            };

            let is_selected = idx == app.active_index;
            let item_style = if is_selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let line = Line::from(vec![
                Span::styled(status_symbol, Style::default().fg(status_color)),
                Span::styled(&service.target.relative_path, item_style),
            ]);

            ListItem::new(line)
        })
        .collect();

    let title = format!(" Services ({}) ", app.command_display);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn render_terminal(frame: &mut Frame, app: &AppState, area: Rect) {
    let active_name = app
        .active_service()
        .map(|s| s.target.relative_path.as_str())
        .unwrap_or("No Service");

    let title = format!(" {} ", active_name);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    if let Some(service) = app.active_service() {
        let screen = service.parser.screen();
        let pseudo_term = PseudoTerminal::new(screen);
        frame.render_widget(pseudo_term, inner_area);
    }
}

fn render_status_bar(frame: &mut Frame, _app: &AppState, area: Rect) {
    let key_style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::DarkGray);
    let bracket_style = Style::default().fg(Color::DarkGray);

    let hints = Line::from(vec![
        Span::raw(" "),
        Span::styled("[", bracket_style),
        Span::styled("↑/↓", key_style),
        Span::styled("] ", bracket_style),
        Span::styled("Select   ", desc_style),
        Span::styled("[", bracket_style),
        Span::styled("Click", key_style),
        Span::styled("] ", bracket_style),
        Span::styled("Focus   ", desc_style),
        Span::styled("[", bracket_style),
        Span::styled("r", key_style),
        Span::styled("] ", bracket_style),
        Span::styled("Restart   ", desc_style),
        Span::styled("[", bracket_style),
        Span::styled("q", key_style),
        Span::styled("] ", bracket_style),
        Span::styled("Quit", desc_style),
    ]);

    let status = Paragraph::new(hints);
    frame.render_widget(status, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::ServiceTarget;
    use ratatui::backend::TestBackend;
    use std::path::PathBuf;

    #[test]
    fn test_ui_rendering_with_test_backend() {
        let target = ServiceTarget {
            name: "test".to_string(),
            relative_path: "apps/backend".to_string(),
            directory: PathBuf::from("apps/backend"),
            command: "bun".to_string(),
            args: vec!["run".to_string(), "dev".to_string()],
        };

        let app = AppState::new(vec![target], "dev".to_string(), 80, 24);
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let areas = render_ui(f, &app);
                assert_eq!(areas.sidebar.width, 28);
                assert_eq!(areas.terminal.width, 52);
            })
            .unwrap();
    }
}
