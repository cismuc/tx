use crate::discovery::ServiceTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceStatus {
    Running,
    Stopped,
    Failed(Option<u32>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalPos {
    pub col: u16,
    pub row: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub start: TerminalPos,
    pub end: TerminalPos,
}

impl Selection {
    pub fn new(col: u16, row: u16) -> Self {
        let pos = TerminalPos { col, row };
        Self { start: pos, end: pos }
    }

    pub fn update(&mut self, col: u16, row: u16) {
        self.end = TerminalPos { col, row };
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    pub fn normalized(&self) -> (TerminalPos, TerminalPos) {
        if (self.start.row, self.start.col) <= (self.end.row, self.end.col) {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        }
    }

    pub fn contains(&self, col: u16, row: u16) -> bool {
        if self.is_empty() {
            return false;
        }
        let (first, last) = self.normalized();
        if row < first.row || row > last.row {
            return false;
        }
        if first.row == last.row {
            row == first.row && col >= first.col && col <= last.col
        } else if row == first.row {
            col >= first.col
        } else if row == last.row {
            col <= last.col
        } else {
            true
        }
    }
}

pub struct ServiceState {
    pub target: ServiceTarget,
    pub status: ServiceStatus,
    pub parser: vt100::Parser,
    pub scroll_offset: usize,
}

impl ServiceState {
    pub fn new(target: ServiceTarget, cols: u16, rows: u16) -> Self {
        let parser = vt100::Parser::new(rows.max(10), cols.max(20), 20000);
        Self {
            target,
            status: ServiceStatus::Stopped,
            parser,
            scroll_offset: 0,
        }
    }

    pub fn feed(&mut self, data: &[u8]) {
        self.parser.process(data);
        self.scroll_offset = self.parser.screen().scrollback();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateStatus {
    Idle,
    Available {
        version: String,
        download_url: String,
        asset_size: u64,
    },
    Downloading {
        version: String,
        percent: u8,
        downloaded: u64,
        total: u64,
    },
    Installing {
        version: String,
    },
    ReadyToRestart {
        version: String,
    },
    Failed {
        error: String,
        timestamp: std::time::Instant,
    },
}

pub struct AppState {
    pub services: Vec<ServiceState>,
    pub active_index: usize,
    pub command_display: String,
    pub should_quit: bool,
    pub focus_terminal: bool,
    pub selection: Option<Selection>,
    pub status_message: Option<(String, std::time::Instant)>,
    pub update_status: UpdateStatus,
}

impl AppState {
    pub fn new(targets: Vec<ServiceTarget>, command_display: String, cols: u16, rows: u16) -> Self {
        let services = targets
            .into_iter()
            .map(|t| ServiceState::new(t, cols, rows))
            .collect();
        Self {
            services,
            active_index: 0,
            command_display,
            should_quit: false,
            focus_terminal: false,
            selection: None,
            status_message: None,
            update_status: UpdateStatus::Idle,
        }
    }

    pub fn select_service(&mut self, index: usize) {
        if index < self.services.len() {
            self.active_index = index;
            if let Some(s) = self.services.get_mut(index) {
                s.parser.set_scrollback(0);
                s.scroll_offset = 0;
            }
            self.clear_selection();
        }
    }

    pub fn select_next(&mut self) {
        if !self.services.is_empty() {
            let next = (self.active_index + 1) % self.services.len();
            self.select_service(next);
        }
    }

    pub fn select_prev(&mut self) {
        if !self.services.is_empty() {
            let prev = if self.active_index == 0 {
                self.services.len() - 1
            } else {
                self.active_index - 1
            };
            self.select_service(prev);
        }
    }

    pub fn feed_output(&mut self, service_index: usize, data: &[u8]) {
        if let Some(service) = self.services.get_mut(service_index) {
            service.feed(data);
        }
    }

    pub fn set_exited(&mut self, service_index: usize, success: bool, code: Option<u32>) {
        if let Some(service) = self.services.get_mut(service_index) {
            service.status = if success {
                ServiceStatus::Stopped
            } else {
                ServiceStatus::Failed(code)
            };
        }
    }

    pub fn set_running(&mut self, service_index: usize) {
        if let Some(service) = self.services.get_mut(service_index) {
            service.status = ServiceStatus::Running;
        }
    }

    pub fn active_service(&self) -> Option<&ServiceState> {
        self.services.get(self.active_index)
    }

    pub fn active_service_mut(&mut self) -> Option<&mut ServiceState> {
        self.services.get_mut(self.active_index)
    }

    pub fn scroll_up(&mut self, amount: usize) {
        if let Some(service) = self.active_service_mut() {
            let current = service.parser.screen().scrollback();
            service.parser.set_scrollback(current.saturating_add(amount));
            service.scroll_offset = service.parser.screen().scrollback();
        }
    }

    pub fn scroll_down(&mut self, amount: usize) {
        if let Some(service) = self.active_service_mut() {
            let current = service.parser.screen().scrollback();
            service.parser.set_scrollback(current.saturating_sub(amount));
            service.scroll_offset = service.parser.screen().scrollback();
        }
    }

    pub fn scroll_to_bottom(&mut self) {
        if let Some(service) = self.active_service_mut() {
            service.parser.set_scrollback(0);
            service.scroll_offset = 0;
        }
    }

    pub fn start_selection(&mut self, col: u16, row: u16) {
        self.selection = Some(Selection::new(col, row));
    }

    pub fn update_selection(&mut self, col: u16, row: u16) {
        if let Some(sel) = self.selection.as_mut() {
            sel.update(col, row);
        }
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    pub fn has_active_selection(&self) -> bool {
        self.selection.as_ref().map_or(false, |s| !s.is_empty())
    }

    pub fn copy_selection(&mut self) -> bool {
        if let Some(sel) = &self.selection {
            if !sel.is_empty() {
                if let Some(service) = self.active_service() {
                    let screen = service.parser.screen();
                    let (_, cols) = screen.size();
                    let (first, last) = sel.normalized();
                    let end_col = (last.col.saturating_add(1)).min(cols);
                    let raw_text = screen.contents_between(first.row, first.col, last.row, end_col);
                    let text = crate::clipboard::clean_copied_text(&raw_text);
                    if !text.is_empty() {
                        let copied = crate::clipboard::copy_to_clipboard(&text);
                        let char_count = text.chars().count();
                        let line_count = text.lines().count();
                        let msg = if line_count > 1 {
                            format!("Copied {} lines ({} chars) to clipboard", line_count, char_count)
                        } else {
                            format!("Copied {} chars to clipboard", char_count)
                        };
                        self.status_message = Some((msg, std::time::Instant::now()));
                        return copied;
                    }
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn mock_target(rel: &str) -> ServiceTarget {
        ServiceTarget {
            name: rel.to_string(),
            relative_path: rel.to_string(),
            directory: PathBuf::from(rel),
            command: "bun".to_string(),
            args: vec!["run".to_string(), "dev".to_string()],
        }
    }

    #[test]
    fn test_app_state_navigation() {
        let targets = vec![
            mock_target("apps/backend"),
            mock_target("apps/frontend"),
            mock_target("apps/admin"),
        ];

        let mut app = AppState::new(targets, "dev".to_string(), 80, 24);
        assert_eq!(app.active_index, 0);

        app.select_next();
        assert_eq!(app.active_index, 1);

        app.select_next();
        assert_eq!(app.active_index, 2);

        // Wraps around to 0
        app.select_next();
        assert_eq!(app.active_index, 0);

        // Wraps back to 2
        app.select_prev();
        assert_eq!(app.active_index, 2);

        app.select_service(1);
        assert_eq!(app.active_index, 1);
    }

    #[test]
    fn test_app_state_feed_ansi_output() {
        let targets = vec![mock_target("apps/backend")];
        let mut app = AppState::new(targets, "dev".to_string(), 80, 24);

        let ansi_text = b"Started server on \x1b[32mhttp://localhost:3000\x1b[0m\r\n";
        app.feed_output(0, ansi_text);

        let service = app.active_service().unwrap();
        let screen = service.parser.screen();
        let contents = screen.contents();
        assert!(
            contents.contains("Started server on http://localhost:3000"),
            "Screen contents did not contain expected text. Got: {}",
            contents
        );
    }

    #[test]
    fn test_scroll_up_and_down_updates_parser() {
        let targets = vec![mock_target("apps/backend")];
        let mut app = AppState::new(targets, "dev".to_string(), 80, 10);

        // Feed 30 lines
        for i in 0..30 {
            app.feed_output(0, format!("line {}\r\n", i).as_bytes());
        }

        assert_eq!(app.active_service().unwrap().parser.screen().scrollback(), 0);

        // Scroll up 5 lines
        app.scroll_up(5);
        assert_eq!(app.active_service().unwrap().parser.screen().scrollback(), 5);
        assert_eq!(app.active_service().unwrap().scroll_offset, 5);

        // Scroll up another 10 lines
        app.scroll_up(10);
        assert_eq!(app.active_service().unwrap().parser.screen().scrollback(), 15);

        // Scroll down 4 lines
        app.scroll_down(4);
        assert_eq!(app.active_service().unwrap().parser.screen().scrollback(), 11);

        // Scroll to bottom
        app.scroll_to_bottom();
        assert_eq!(app.active_service().unwrap().parser.screen().scrollback(), 0);
    }

    #[test]
    fn test_selection_contains_and_copy() {
        let targets = vec![mock_target("apps/backend")];
        let mut app = AppState::new(targets, "dev".to_string(), 80, 10);
        app.feed_output(0, b"Hello World!\r\nSecond Line\r\n");

        app.start_selection(0, 0);
        app.update_selection(10, 0);

        assert!(app.has_active_selection());
        let sel = app.selection.as_ref().unwrap();
        assert!(sel.contains(0, 0));
        assert!(sel.contains(5, 0));
        assert!(sel.contains(10, 0));
        assert!(!sel.contains(11, 0));
        assert!(!sel.contains(0, 1));

        let copied = app.copy_selection();
        assert!(copied);
        assert!(app.status_message.is_some());
    }
}
