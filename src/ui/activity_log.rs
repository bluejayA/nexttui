use std::time::Instant;

/// A single activity entry representing an operation result.
pub struct ActivityEntry {
    pub timestamp: Instant,
    pub operation: String,
    pub resource_name: String,
    pub success: bool,
    pub message: String,
    pub read: bool,
}

/// Ordered log of recent activity entries (newest first, max 20).
pub struct ActivityLog {
    entries: Vec<ActivityEntry>,
}

const MAX_ENTRIES: usize = 20;

impl ActivityLog {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn push(&mut self, entry: ActivityEntry) {
        self.entries.insert(0, entry);
        if self.entries.len() > MAX_ENTRIES {
            self.entries.truncate(MAX_ENTRIES);
        }
    }

    pub fn entries(&self) -> &[ActivityEntry] {
        &self.entries
    }

    pub fn unread_error_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| !e.success && !e.read)
            .count()
    }

    pub fn mark_all_read(&mut self) {
        for entry in &mut self.entries {
            entry.read = true;
        }
    }

    /// Export all entries to a tab-separated text file.
    pub fn export_to_file(&self, path: &std::path::Path) -> std::io::Result<()> {
        use std::io::Write;
        let mut buf = Vec::new();
        if self.entries.is_empty() {
            writeln!(buf, "(no entries)")?;
        } else {
            let now = Instant::now();
            for entry in &self.entries {
                let elapsed = now.duration_since(entry.timestamp);
                let time_str = format_relative_time(elapsed);
                let icon = if entry.success {
                    "\u{2713}"
                } else {
                    "\u{2717}"
                };
                if entry.success || entry.message.is_empty() {
                    writeln!(
                        buf,
                        "{time_str}\t{icon}\t{}\t{}",
                        entry.operation, entry.resource_name
                    )?;
                } else {
                    writeln!(
                        buf,
                        "{time_str}\t{icon}\t{}\t{}\t{}",
                        entry.operation, entry.resource_name, entry.message
                    )?;
                }
            }
        }
        std::fs::write(path, buf)
    }
}

impl Default for ActivityLog {
    fn default() -> Self {
        Self::new()
    }
}

use std::time::Duration;

/// Format a Duration as a human-readable relative time string.
pub fn format_relative_time(elapsed: Duration) -> String {
    let secs = elapsed.as_secs();
    if secs < 1 {
        "<1s".to_string()
    } else if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h", secs / 3600)
    }
}

/// Popup widget for displaying the activity log overlay.
pub struct ActivityLogPopup {
    scroll_offset: usize,
}

impl ActivityLogPopup {
    pub fn new() -> Self {
        Self { scroll_offset: 0 }
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn scroll_down(&mut self, total: usize) {
        if total > 0 && self.scroll_offset < total.saturating_sub(1) {
            self.scroll_offset += 1;
        }
    }

    pub fn reset_scroll(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Render the activity log as a centered overlay popup.
    pub fn render(
        &self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        entries: &[ActivityEntry],
    ) {
        use ratatui::layout::Rect;
        use ratatui::style::{Color, Style};
        use ratatui::text::{Line, Span};
        use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

        // Centered popup: 80% width, 50% height
        let popup_width = area.width * 80 / 100;
        let popup_height = area.height * 50 / 100;
        let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;
        let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

        frame.render_widget(Clear, popup_area);

        let block = Block::default()
            .title(" Activity Log  (w:write to file) ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Yellow));

        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        let visible_height = inner.height as usize;
        let now = Instant::now();

        let lines: Vec<Line<'_>> = entries
            .iter()
            .skip(self.scroll_offset)
            .take(visible_height)
            .map(|entry| {
                let elapsed = now.duration_since(entry.timestamp);
                let time_str = format_relative_time(elapsed);
                let (icon, icon_color) = if entry.success {
                    ("\u{2713}", Color::Green)
                } else {
                    ("\u{2717}", Color::Red)
                };
                let mut spans = vec![
                    Span::styled(
                        format!("[{:>4}] ", time_str),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(format!("{icon} "), Style::default().fg(icon_color)),
                    Span::raw(format!("{} {}", entry.operation, entry.resource_name)),
                ];
                if !entry.success && !entry.message.is_empty() {
                    spans.push(Span::styled(
                        format!(" \u{2014} {}", entry.message),
                        Style::default().fg(Color::Red),
                    ));
                }
                Line::from(spans)
            })
            .collect();

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }
}

impl Default for ActivityLogPopup {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn make_entry(operation: &str, success: bool, read: bool) -> ActivityEntry {
        ActivityEntry {
            timestamp: Instant::now(),
            operation: operation.to_string(),
            resource_name: "test-resource".to_string(),
            success,
            message: if success {
                String::new()
            } else {
                "error occurred".to_string()
            },
            read,
        }
    }

    #[test]
    fn test_new_creates_empty_log() {
        let log = ActivityLog::new();
        assert!(log.entries().is_empty());
    }

    #[test]
    fn test_push_adds_entry() {
        let mut log = ActivityLog::new();
        log.push(make_entry("DeleteServer", true, false));
        assert_eq!(log.entries().len(), 1);
        assert_eq!(log.entries()[0].operation, "DeleteServer");
    }

    #[test]
    fn test_push_newest_first() {
        let mut log = ActivityLog::new();
        log.push(make_entry("First", true, false));
        log.push(make_entry("Second", true, false));
        assert_eq!(log.entries()[0].operation, "Second");
        assert_eq!(log.entries()[1].operation, "First");
    }

    #[test]
    fn test_push_fifo_cap_20() {
        let mut log = ActivityLog::new();
        for i in 0..25 {
            log.push(make_entry(&format!("Op{i}"), true, false));
        }
        assert_eq!(log.entries().len(), 20);
        // Newest is Op24 (at front), oldest kept is Op5
        assert_eq!(log.entries()[0].operation, "Op24");
        assert_eq!(log.entries()[19].operation, "Op5");
    }

    #[test]
    fn test_unread_error_count() {
        let mut log = ActivityLog::new();
        log.push(make_entry("Op1", false, false)); // unread error
        log.push(make_entry("Op2", false, true)); // read error
        log.push(make_entry("Op3", true, false)); // success (not counted)
        assert_eq!(log.unread_error_count(), 1);
    }

    #[test]
    fn test_unread_error_count_zero_when_all_success() {
        let mut log = ActivityLog::new();
        log.push(make_entry("Op1", true, false));
        log.push(make_entry("Op2", true, false));
        assert_eq!(log.unread_error_count(), 0);
    }

    #[test]
    fn test_mark_all_read() {
        let mut log = ActivityLog::new();
        log.push(make_entry("Op1", false, false));
        log.push(make_entry("Op2", false, false));
        assert_eq!(log.unread_error_count(), 2);
        log.mark_all_read();
        assert_eq!(log.unread_error_count(), 0);
        assert!(log.entries().iter().all(|e| e.read));
    }

    // --- Unit 2: format_relative_time ---

    #[test]
    fn test_format_relative_time_sub_second() {
        assert_eq!(format_relative_time(Duration::from_millis(500)), "<1s");
        assert_eq!(format_relative_time(Duration::from_millis(0)), "<1s");
    }

    #[test]
    fn test_format_relative_time_seconds() {
        assert_eq!(format_relative_time(Duration::from_secs(3)), "3s");
        assert_eq!(format_relative_time(Duration::from_secs(59)), "59s");
    }

    #[test]
    fn test_format_relative_time_minutes() {
        assert_eq!(format_relative_time(Duration::from_secs(60)), "1m");
        assert_eq!(format_relative_time(Duration::from_secs(120)), "2m");
        assert_eq!(format_relative_time(Duration::from_secs(3599)), "59m");
    }

    #[test]
    fn test_format_relative_time_hours() {
        assert_eq!(format_relative_time(Duration::from_secs(3600)), "1h");
        assert_eq!(format_relative_time(Duration::from_secs(7200)), "2h");
        assert_eq!(format_relative_time(Duration::from_secs(10800)), "3h");
    }

    // --- Unit 2: ActivityLogPopup ---

    #[test]
    fn test_popup_new_scroll_offset_zero() {
        let popup = ActivityLogPopup::new();
        assert_eq!(popup.scroll_offset(), 0);
    }

    #[test]
    fn test_popup_scroll_up_at_zero_stays_zero() {
        let mut popup = ActivityLogPopup::new();
        popup.scroll_up();
        assert_eq!(popup.scroll_offset(), 0);
    }

    #[test]
    fn test_popup_scroll_down_increases() {
        let mut popup = ActivityLogPopup::new();
        popup.scroll_down(10);
        assert_eq!(popup.scroll_offset(), 1);
        popup.scroll_down(10);
        assert_eq!(popup.scroll_offset(), 2);
    }

    #[test]
    fn test_popup_scroll_down_bounded_by_total() {
        let mut popup = ActivityLogPopup::new();
        // total=3 → max offset is 2 (total-1)
        popup.scroll_down(3);
        popup.scroll_down(3);
        popup.scroll_down(3);
        popup.scroll_down(3);
        assert_eq!(popup.scroll_offset(), 2);
    }

    #[test]
    fn test_popup_scroll_down_zero_total_no_op() {
        let mut popup = ActivityLogPopup::new();
        popup.scroll_down(0);
        assert_eq!(popup.scroll_offset(), 0);
    }

    #[test]
    fn test_export_to_file_writes_entries() {
        let dir = std::env::temp_dir().join("nexttui-test-export");
        let _ = std::fs::remove_file(&dir);
        let mut log = ActivityLog::new();
        log.push(make_entry("Delete", false, false));
        log.push(make_entry("Create", true, false));
        let result = log.export_to_file(&dir);
        assert!(result.is_ok());
        let content = std::fs::read_to_string(&dir).expect("file should exist");
        assert!(content.contains("Create"));
        assert!(content.contains("Delete"));
        assert!(content.contains("\u{2713}")); // ✓
        assert!(content.contains("\u{2717}")); // ✗
        let _ = std::fs::remove_file(&dir);
    }

    #[test]
    fn test_export_empty_log() {
        let dir = std::env::temp_dir().join("nexttui-test-export-empty");
        let _ = std::fs::remove_file(&dir);
        let log = ActivityLog::new();
        let result = log.export_to_file(&dir);
        assert!(result.is_ok());
        let content = std::fs::read_to_string(&dir).expect("file should exist");
        assert!(content.contains("(no entries)"));
        let _ = std::fs::remove_file(&dir);
    }

    #[test]
    fn test_popup_reset_scroll() {
        let mut popup = ActivityLogPopup::new();
        popup.scroll_down(10);
        popup.scroll_down(10);
        assert_eq!(popup.scroll_offset(), 2);
        popup.reset_scroll();
        assert_eq!(popup.scroll_offset(), 0);
    }
}
