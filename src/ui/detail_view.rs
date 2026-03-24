use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::action::Action;
use crate::models::common::Route;
use crate::ui::resource_list::RowStyleHint;

pub struct DetailData {
    pub title: String,
    pub sections: Vec<DetailSection>,
}

pub struct DetailSection {
    pub name: String,
    pub fields: Vec<DetailField>,
}

pub enum DetailField {
    KeyValue {
        key: String,
        value: String,
        style: Option<RowStyleHint>,
    },
    NestedTable {
        label: String,
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    ResourceLink {
        key: String,
        display: String,
        target_route: Route,
        target_id: String,
    },
}

pub struct DetailView {
    data: Option<DetailData>,
    scroll_offset: usize,
    focused_link_index: usize,
    links: Vec<(Route, String)>,
    loading: bool,
}

impl DetailView {
    pub fn new() -> Self {
        Self {
            data: None,
            scroll_offset: 0,
            focused_link_index: 0,
            links: Vec::new(),
            loading: false,
        }
    }

    pub fn set_data(&mut self, data: DetailData) {
        self.links = data
            .sections
            .iter()
            .flat_map(|s| s.fields.iter())
            .filter_map(|f| match f {
                DetailField::ResourceLink {
                    target_route,
                    target_id,
                    ..
                } => Some((*target_route, target_id.clone())),
                _ => None,
            })
            .collect();
        self.data = Some(data);
        self.scroll_offset = 0;
        self.focused_link_index = 0;
    }

    pub fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }

    pub fn clear(&mut self) {
        self.data = None;
        self.links.clear();
        self.scroll_offset = 0;
        self.focused_link_index = 0;
        self.loading = false;
    }

    pub fn has_data(&self) -> bool {
        self.data.is_some()
    }

    pub fn link_count(&self) -> usize {
        self.links.len()
    }

    pub fn focused_link_index(&self) -> usize {
        self.focused_link_index
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                // Clamp to prevent u16 overflow on render
                if self.scroll_offset < u16::MAX as usize {
                    self.scroll_offset += 1;
                }
                None
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
                None
            }
            KeyCode::Tab => {
                if !self.links.is_empty() {
                    self.focused_link_index =
                        (self.focused_link_index + 1) % self.links.len();
                }
                None
            }
            KeyCode::Enter => {
                self.links.get(self.focused_link_index).map(|(route, id)| {
                    Action::NavigateToResource {
                        route: *route,
                        id: id.clone(),
                    }
                })
            }
            KeyCode::Esc => Some(Action::Back),
            _ => None,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if self.loading {
            let widget = Paragraph::new(Line::from(Span::styled(
                "Loading...",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )))
            .alignment(ratatui::layout::Alignment::Center);
            frame.render_widget(widget, area);
            return;
        }

        let data = match &self.data {
            Some(d) => d,
            None => {
                let widget = Paragraph::new("No data");
                frame.render_widget(widget, area);
                return;
            }
        };

        let mut lines = vec![Line::from(Span::styled(
            &data.title,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ))];
        lines.push(Line::from(""));

        for section in &data.sections {
            lines.push(Line::from(Span::styled(
                format!("-- {} ", section.name),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )));

            // Calculate max key width for aligned columns within this section
            let key_width = section
                .fields
                .iter()
                .map(|f| match f {
                    DetailField::KeyValue { key, .. } => key.len(),
                    DetailField::ResourceLink { key, .. } => key.len(),
                    DetailField::NestedTable { label, .. } => label.len(),
                })
                .max()
                .unwrap_or(0);

            for field in &section.fields {
                match field {
                    DetailField::KeyValue { key, value, style } => {
                        let val_color = style
                            .as_ref()
                            .map(|s| s.color())
                            .unwrap_or(Color::White);
                        lines.push(Line::from(vec![
                            Span::styled(
                                format!("  {:>width$}: ", key, width = key_width),
                                Style::default().fg(Color::LightBlue),
                            ),
                            Span::styled(value, Style::default().fg(val_color)),
                        ]));
                    }
                    DetailField::NestedTable {
                        label,
                        columns,
                        rows,
                    } => {
                        // Calculate per-column max width from headers + all rows
                        let col_count = columns.len();
                        let mut col_widths: Vec<usize> = columns.iter().map(|c| c.len()).collect();
                        for row in rows {
                            for (i, cell) in row.iter().enumerate() {
                                if i < col_widths.len() {
                                    col_widths[i] = col_widths[i].max(cell.len());
                                }
                            }
                        }

                        lines.push(Line::from(Span::styled(
                            format!("  {:>width$}:", label, width = key_width),
                            Style::default().fg(Color::Gray),
                        )));
                        let col_padding = " ".repeat(key_width + 4);

                        // Render header with fixed-width columns
                        let header: String = columns
                            .iter()
                            .enumerate()
                            .map(|(i, c)| format!("{:<w$}", c, w = col_widths.get(i).copied().unwrap_or(c.len())))
                            .collect::<Vec<_>>()
                            .join(" | ");
                        lines.push(Line::from(Span::styled(
                            format!("{col_padding}{header}"),
                            Style::default().add_modifier(Modifier::UNDERLINED),
                        )));

                        // Render rows with same fixed-width columns
                        for row in rows {
                            let row_str: String = (0..col_count)
                                .map(|i| {
                                    let cell = row.get(i).map(|s| s.as_str()).unwrap_or("");
                                    let w = col_widths.get(i).copied().unwrap_or(0);
                                    format!("{:<w$}", cell, w = w)
                                })
                                .collect::<Vec<_>>()
                                .join(" | ");
                            lines.push(Line::from(format!("{col_padding}{row_str}")));
                        }
                    }
                    DetailField::ResourceLink { key, display, .. } => {
                        lines.push(Line::from(vec![
                            Span::styled(
                                format!("  {:>width$}: ", key, width = key_width),
                                Style::default().fg(Color::LightBlue),
                            ),
                            Span::styled(
                                format!("[{display}]"),
                                Style::default()
                                    .fg(Color::Cyan)
                                    .add_modifier(Modifier::UNDERLINED),
                            ),
                        ]));
                    }
                }
            }
            lines.push(Line::from(""));
        }

        let widget = Paragraph::new(lines).scroll((self.scroll_offset as u16, 0));
        frame.render_widget(widget, area);
    }
}

impl Default for DetailView {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_data() -> DetailData {
        DetailData {
            title: "Server: web-01".into(),
            sections: vec![
                DetailSection {
                    name: "Basic Info".into(),
                    fields: vec![
                        DetailField::KeyValue {
                            key: "ID".into(),
                            value: "srv-123".into(),
                            style: None,
                        },
                        DetailField::KeyValue {
                            key: "Status".into(),
                            value: "ACTIVE".into(),
                            style: Some(RowStyleHint::Active),
                        },
                        DetailField::ResourceLink {
                            key: "Image".into(),
                            display: "Ubuntu 22.04".into(),
                            target_route: Route::Images,
                            target_id: "img-456".into(),
                        },
                    ],
                },
                DetailSection {
                    name: "Network".into(),
                    fields: vec![DetailField::ResourceLink {
                        key: "Network".into(),
                        display: "private-net".into(),
                        target_route: Route::Networks,
                        target_id: "net-789".into(),
                    }],
                },
            ],
        }
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::from(code)
    }

    #[test]
    fn test_set_data_extracts_links() {
        let mut view = DetailView::new();
        view.set_data(sample_data());
        assert!(view.has_data());
        assert_eq!(view.link_count(), 2);
    }

    #[test]
    fn test_clear() {
        let mut view = DetailView::new();
        view.set_data(sample_data());
        view.clear();
        assert!(!view.has_data());
        assert_eq!(view.link_count(), 0);
    }

    #[test]
    fn test_tab_cycles_links() {
        let mut view = DetailView::new();
        view.set_data(sample_data());
        assert_eq!(view.focused_link_index(), 0);
        view.handle_key(key(KeyCode::Tab));
        assert_eq!(view.focused_link_index(), 1);
        view.handle_key(key(KeyCode::Tab));
        assert_eq!(view.focused_link_index(), 0); // wraps
    }

    #[test]
    fn test_enter_navigates_to_link_with_id() {
        let mut view = DetailView::new();
        view.set_data(sample_data());
        let action = view.handle_key(key(KeyCode::Enter));
        match action {
            Some(Action::NavigateToResource { route, id }) => {
                assert_eq!(route, Route::Images);
                assert_eq!(id, "img-456");
            }
            _ => panic!("expected NavigateToResource"),
        }
    }

    #[test]
    fn test_esc_goes_back() {
        let mut view = DetailView::new();
        view.set_data(sample_data());
        let action = view.handle_key(key(KeyCode::Esc));
        assert!(matches!(action, Some(Action::Back)));
    }
}
