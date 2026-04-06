use std::collections::VecDeque;

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

const MAX_ENTRIES: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Success,
    Warning,
    Error,
}

impl LogLevel {
    fn style(self) -> Style {
        match self {
            Self::Info => Style::default().fg(Color::White),
            Self::Success => Style::default().fg(Color::Green),
            Self::Warning => Style::default().fg(Color::Yellow),
            Self::Error => Style::default().fg(Color::Red),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Success => " OK ",
            Self::Warning => "WARN",
            Self::Error => "ERR ",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub message: String,
}

pub struct LogPanel {
    entries: VecDeque<LogEntry>,
}

impl Default for LogPanel {
    fn default() -> Self { Self::new() }
}

impl LogPanel {
    pub fn new() -> Self {
        Self {
            entries: VecDeque::new(),
        }
    }

    pub fn push(&mut self, timestamp: String, level: LogLevel, message: String) {
        if self.entries.len() >= MAX_ENTRIES {
            self.entries.pop_front();
        }
        self.entries.push_back(LogEntry {
            timestamp,
            level,
            message,
        });
    }

    pub fn entries(&self) -> &VecDeque<LogEntry> {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray))
            .title("  Log  ");
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let visible = inner.height as usize;
        let skip = if self.entries.len() > visible {
            self.entries.len() - visible
        } else {
            0
        };

        let lines: Vec<Line> = self
            .entries
            .iter()
            .skip(skip)
            .take(visible)
            .map(|entry| {
                Line::from(vec![
                    Span::styled(&entry.timestamp, Style::default().fg(Color::Cyan)),
                    Span::raw(" "),
                    Span::styled(entry.level.label(), entry.level.style()),
                    Span::raw(" "),
                    Span::styled(&entry.message, entry.level.style()),
                ])
            })
            .collect();

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_panel_push_and_len() {
        let mut panel = LogPanel::new();
        assert!(panel.is_empty());

        panel.push("10:00".into(), LogLevel::Info, "Started".into());
        assert_eq!(panel.len(), 1);

        panel.push("10:01".into(), LogLevel::Error, "Failed".into());
        assert_eq!(panel.len(), 2);
    }

    #[test]
    fn test_log_panel_ring_buffer_limit() {
        let mut panel = LogPanel::new();
        for i in 0..250 {
            panel.push(format!("{i}"), LogLevel::Info, format!("msg-{i}"));
        }
        assert_eq!(panel.len(), MAX_ENTRIES); // capped at 200
        // Oldest entries should be dropped
        assert_eq!(panel.entries().front().unwrap().timestamp, "50");
        assert_eq!(panel.entries().back().unwrap().timestamp, "249");
    }

    #[test]
    fn test_log_panel_clear() {
        let mut panel = LogPanel::new();
        panel.push("10:00".into(), LogLevel::Info, "test".into());
        panel.clear();
        assert!(panel.is_empty());
    }

    #[test]
    fn test_log_level_labels() {
        assert_eq!(LogLevel::Info.label(), "INFO");
        assert_eq!(LogLevel::Success.label(), " OK ");
        assert_eq!(LogLevel::Warning.label(), "WARN");
        assert_eq!(LogLevel::Error.label(), "ERR ");
    }
}
