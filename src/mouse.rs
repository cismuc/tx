use crate::state::AppState;
use crate::ui::LayoutAreas;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

pub fn handle_mouse_event(event: MouseEvent, areas: &LayoutAreas, app: &mut AppState) {
    let col = event.column;
    let row = event.row;

    match event.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if is_inside(col, row, areas.sidebar) {
                app.focus_terminal = false;
                app.clear_selection();

                // Account for border (1 line at top)
                let content_top = areas.sidebar.y + 1;
                let content_bottom = areas.sidebar.y + areas.sidebar.height.saturating_sub(1);

                if row >= content_top && row < content_bottom {
                    let clicked_index = (row - content_top) as usize;
                    if clicked_index < app.services.len() {
                        app.select_service(clicked_index);
                    }
                }
            } else if is_inside(col, row, areas.terminal) {
                app.focus_terminal = true;
                let inner = terminal_inner(areas.terminal);
                if is_inside(col, row, inner) {
                    let p_col = col.saturating_sub(inner.x);
                    let p_row = row.saturating_sub(inner.y);
                    app.start_selection(p_col, p_row);
                } else {
                    app.clear_selection();
                }
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if app.focus_terminal && is_inside(col, row, areas.terminal) {
                let inner = terminal_inner(areas.terminal);
                let p_col = col.saturating_sub(inner.x).min(inner.width.saturating_sub(1));
                let p_row = row.saturating_sub(inner.y).min(inner.height.saturating_sub(1));
                app.update_selection(p_col, p_row);

                // Auto-scroll when dragging near top or bottom border
                if row <= inner.y && inner.height > 0 {
                    app.scroll_up(1);
                } else if row >= inner.y.saturating_add(inner.height).saturating_sub(1) {
                    app.scroll_down(1);
                }
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            if app.focus_terminal && app.has_active_selection() {
                app.copy_selection();
            }
        }
        MouseEventKind::ScrollUp => {
            if is_inside(col, row, areas.sidebar) {
                app.select_prev();
            } else if is_inside(col, row, areas.terminal) {
                app.scroll_up(3);
            }
        }
        MouseEventKind::ScrollDown => {
            if is_inside(col, row, areas.sidebar) {
                app.select_next();
            } else if is_inside(col, row, areas.terminal) {
                app.scroll_down(3);
            }
        }
        _ => {}
    }
}

pub fn terminal_inner(area: Rect) -> Rect {
    Rect {
        x: area.x.saturating_add(1),
        y: area.y.saturating_add(1),
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    }
}

pub fn is_inside(col: u16, row: u16, rect: Rect) -> bool {
    col >= rect.x
        && col < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::ServiceTarget;
    use std::path::PathBuf;

    fn mock_app() -> AppState {
        let targets = vec![
            ServiceTarget {
                name: "b".to_string(),
                relative_path: "apps/backend".to_string(),
                directory: PathBuf::from("apps/backend"),
                command: "bun".to_string(),
                args: vec!["run".to_string(), "dev".to_string()],
            },
            ServiceTarget {
                name: "f".to_string(),
                relative_path: "apps/frontend".to_string(),
                directory: PathBuf::from("apps/frontend"),
                command: "bun".to_string(),
                args: vec!["run".to_string(), "dev".to_string()],
            },
        ];
        AppState::new(targets, "dev".to_string(), 80, 24)
    }

    fn mock_areas() -> LayoutAreas {
        LayoutAreas {
            sidebar: Rect {
                x: 0,
                y: 0,
                width: 28,
                height: 23,
            },
            terminal: Rect {
                x: 28,
                y: 0,
                width: 52,
                height: 23,
            },
            status_bar: Rect {
                x: 0,
                y: 23,
                width: 80,
                height: 1,
            },
        }
    }

    #[test]
    fn test_mouse_click_selects_sidebar_item() {
        let mut app = mock_app();
        let areas = mock_areas();

        assert_eq!(app.active_index, 0);

        // Click row 1 is item 0 (inside border)
        let click_item_0 = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 5,
            row: 1,
            modifiers: crossterm::event::KeyModifiers::NONE,
        };
        handle_mouse_event(click_item_0, &areas, &mut app);
        assert_eq!(app.active_index, 0);

        // Click row 2 is item 1 (apps/frontend)
        let click_item_1 = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 5,
            row: 2,
            modifiers: crossterm::event::KeyModifiers::NONE,
        };
        handle_mouse_event(click_item_1, &areas, &mut app);
        assert_eq!(app.active_index, 1);
    }

    #[test]
    fn test_mouse_click_on_border_or_empty_does_not_change_selection() {
        let mut app = mock_app();
        let areas = mock_areas();

        // Click row 0 (top border)
        let click_border = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 5,
            row: 0,
            modifiers: crossterm::event::KeyModifiers::NONE,
        };
        handle_mouse_event(click_border, &areas, &mut app);
        assert_eq!(app.active_index, 0);

        // Click row 10 (beyond the 2 items)
        let click_empty = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 5,
            row: 10,
            modifiers: crossterm::event::KeyModifiers::NONE,
        };
        handle_mouse_event(click_empty, &areas, &mut app);
        assert_eq!(app.active_index, 0);
    }

    #[test]
    fn test_mouse_scroll_over_sidebar() {
        let mut app = mock_app();
        let areas = mock_areas();

        let scroll_down = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 10,
            row: 5,
            modifiers: crossterm::event::KeyModifiers::NONE,
        };
        handle_mouse_event(scroll_down, &areas, &mut app);
        assert_eq!(app.active_index, 1);

        let scroll_up = MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 10,
            row: 5,
            modifiers: crossterm::event::KeyModifiers::NONE,
        };
        handle_mouse_event(scroll_up, &areas, &mut app);
        assert_eq!(app.active_index, 0);
    }

    #[test]
    fn test_mouse_drag_in_terminal_creates_selection() {
        let mut app = mock_app();
        let areas = mock_areas();

        let m_down = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 30, // inner x = 1
            row: 2,    // inner y = 1
            modifiers: crossterm::event::KeyModifiers::NONE,
        };
        handle_mouse_event(m_down, &areas, &mut app);
        assert!(app.focus_terminal);
        assert!(app.selection.is_some());
        assert!(app.selection.as_ref().unwrap().is_empty());

        let m_drag = MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: 35, // inner x = 6
            row: 2,    // inner y = 1
            modifiers: crossterm::event::KeyModifiers::NONE,
        };
        handle_mouse_event(m_drag, &areas, &mut app);
        assert!(app.has_active_selection());
        let sel = app.selection.as_ref().unwrap();
        assert_eq!(sel.start.col, 1);
        assert_eq!(sel.start.row, 1);
        assert_eq!(sel.end.col, 6);
        assert_eq!(sel.end.row, 1);

        let m_up = MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: 35,
            row: 2,
            modifiers: crossterm::event::KeyModifiers::NONE,
        };
        handle_mouse_event(m_up, &areas, &mut app);
        assert!(app.has_active_selection());
    }
}
