use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};

use super::theme::Theme;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row as TuiRow, Table, TableState};
use ratatui::Frame;

use crate::action::Action;

#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub name: String,
    pub width: ColumnWidth,
    pub alignment: ratatui::layout::Alignment,
}

#[derive(Debug, Clone)]
pub enum ColumnWidth {
    Fixed(u16),
    Percent(u16),
    Min(u16),
}

impl ColumnWidth {
    pub fn to_constraint(&self) -> Constraint {
        match self {
            ColumnWidth::Fixed(w) => Constraint::Length(*w),
            ColumnWidth::Percent(p) => Constraint::Percentage(*p),
            ColumnWidth::Min(m) => Constraint::Min(*m),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Row {
    pub cells: Vec<String>,
    pub id: String,
    pub style_hint: Option<RowStyleHint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowStyleHint {
    Normal,
    Active,
    Error,
    Warning,
    Disabled,
}

impl RowStyleHint {
    pub fn style(&self) -> Style {
        match self {
            RowStyleHint::Normal => Style::default().fg(Color::White),
            RowStyleHint::Active => Theme::done(),
            RowStyleHint::Error => Theme::error(),
            RowStyleHint::Warning => Theme::warning(),
            RowStyleHint::Disabled => Theme::disabled(),
        }
    }
}

pub struct ResourceList {
    columns: Vec<ColumnDef>,
    rows: Vec<Row>,
    filtered_indices: Vec<usize>,
    selected: usize,
    loading: bool,
    search_term: Option<String>,
}

impl ResourceList {
    pub fn new(columns: Vec<ColumnDef>) -> Self {
        Self {
            columns,
            rows: Vec::new(),
            filtered_indices: Vec::new(),
            selected: 0,
            loading: false,
            search_term: None,
        }
    }

    pub fn set_rows(&mut self, rows: Vec<Row>) {
        self.filtered_indices = (0..rows.len()).collect();
        self.rows = rows;
        self.selected = 0;
    }

    pub fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }

    pub fn selected_id(&self) -> Option<&str> {
        self.filtered_indices
            .get(self.selected)
            .and_then(|&i| self.rows.get(i))
            .map(|r| r.id.as_str())
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn visible_count(&self) -> usize {
        self.filtered_indices.len()
    }

    pub fn total_count(&self) -> usize {
        self.rows.len()
    }

    pub fn apply_filter(&mut self, term: &str) {
        if term.is_empty() {
            self.clear_filter();
            return;
        }
        let lower = term.to_lowercase();
        self.filtered_indices = self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.cells.iter().any(|c| c.to_lowercase().contains(&lower)))
            .map(|(i, _)| i)
            .collect();
        self.search_term = Some(term.to_string());
        self.selected = 0;
    }

    pub fn clear_filter(&mut self) {
        self.filtered_indices = (0..self.rows.len()).collect();
        self.search_term = None;
        self.selected = 0;
    }

    /// Handle common list navigation keys (j/k/g/G). Returns true if the key was consumed.
    pub fn handle_nav_key(&mut self, key: KeyEvent) -> bool {
        let max = self.filtered_indices.len().saturating_sub(1);
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if self.selected < max {
                    self.selected += 1;
                }
                true
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                true
            }
            KeyCode::Char('G') => {
                self.selected = max;
                true
            }
            KeyCode::Char('g') => {
                self.selected = 0;
                true
            }
            _ => false,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        if self.handle_nav_key(key) {
            return None;
        }
        match key.code {
            KeyCode::Enter => self.selected_id().map(|id| Action::SelectResource {
                id: id.to_string(),
            }),
            _ => None,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if self.loading {
            let spinner = Paragraph::new(Line::from(Span::styled(
                "Loading...",
                Theme::active(),
            )))
            .alignment(ratatui::layout::Alignment::Center);
            frame.render_widget(spinner, area);
            return;
        }

        if self.filtered_indices.is_empty() {
            let empty = Paragraph::new(Line::from(Span::styled(
                "No items found",
                Theme::waiting(),
            )))
            .alignment(ratatui::layout::Alignment::Center);
            frame.render_widget(empty, area);
            return;
        }

        let header_cells: Vec<Cell> = self
            .columns
            .iter()
            .map(|c| {
                Cell::from(Span::styled(
                    c.name.clone(),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                ))
            })
            .collect();
        let header = TuiRow::new(header_cells);

        let widths: Vec<Constraint> = self.columns.iter().map(|c| c.width.to_constraint()).collect();

        let data_rows: Vec<TuiRow> = self
            .filtered_indices
            .iter()
            .enumerate()
            .map(|(vi, &ri)| {
                let row = &self.rows[ri];
                let base_style = row
                    .style_hint
                    .as_ref()
                    .map(|h| h.style())
                    .unwrap_or_else(|| Style::default().fg(Color::White));
                let style = if vi == self.selected {
                    base_style.patch(Theme::highlight())
                } else {
                    base_style
                };
                let cells: Vec<Cell> = row.cells.iter().map(|c| Cell::from(c.as_str())).collect();
                TuiRow::new(cells).style(style)
            })
            .collect();

        let table = Table::new(data_rows, &widths)
            .header(header)
            .block(Block::default().borders(Borders::NONE));

        let mut state = TableState::default();
        state.select(Some(self.selected));
        frame.render_stateful_widget(table, area, &mut state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_rows() -> Vec<Row> {
        vec![
            Row {
                id: "srv-1".into(),
                cells: vec!["web-01".into(), "ACTIVE".into()],
                style_hint: Some(RowStyleHint::Active),
            },
            Row {
                id: "srv-2".into(),
                cells: vec!["web-02".into(), "ERROR".into()],
                style_hint: Some(RowStyleHint::Error),
            },
            Row {
                id: "srv-3".into(),
                cells: vec!["db-01".into(), "SHUTOFF".into()],
                style_hint: Some(RowStyleHint::Disabled),
            },
        ]
    }

    fn sample_columns() -> Vec<ColumnDef> {
        vec![
            ColumnDef {
                name: "Name".into(),
                width: ColumnWidth::Percent(50),
                alignment: ratatui::layout::Alignment::Left,
            },
            ColumnDef {
                name: "Status".into(),
                width: ColumnWidth::Percent(50),
                alignment: ratatui::layout::Alignment::Left,
            },
        ]
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::from(code)
    }

    #[test]
    fn test_set_rows_and_counts() {
        let mut list = ResourceList::new(sample_columns());
        list.set_rows(sample_rows());
        assert_eq!(list.total_count(), 3);
        assert_eq!(list.visible_count(), 3);
        assert_eq!(list.selected_index(), 0);
    }

    #[test]
    fn test_selected_id() {
        let mut list = ResourceList::new(sample_columns());
        list.set_rows(sample_rows());
        assert_eq!(list.selected_id(), Some("srv-1"));
    }

    #[test]
    fn test_navigate_j_k() {
        let mut list = ResourceList::new(sample_columns());
        list.set_rows(sample_rows());
        list.handle_key(key(KeyCode::Char('j')));
        assert_eq!(list.selected_index(), 1);
        assert_eq!(list.selected_id(), Some("srv-2"));

        list.handle_key(key(KeyCode::Char('k')));
        assert_eq!(list.selected_index(), 0);
    }

    #[test]
    fn test_navigate_g_and_shift_g() {
        let mut list = ResourceList::new(sample_columns());
        list.set_rows(sample_rows());
        list.handle_key(key(KeyCode::Char('G')));
        assert_eq!(list.selected_index(), 2);

        list.handle_key(key(KeyCode::Char('g')));
        assert_eq!(list.selected_index(), 0);
    }

    #[test]
    fn test_enter_selects() {
        let mut list = ResourceList::new(sample_columns());
        list.set_rows(sample_rows());
        list.handle_key(key(KeyCode::Char('j')));
        let action = list.handle_key(key(KeyCode::Enter));
        match action {
            Some(Action::SelectResource { id }) => assert_eq!(id, "srv-2"),
            _ => panic!("expected SelectResource"),
        }
    }

    #[test]
    fn test_apply_filter() {
        let mut list = ResourceList::new(sample_columns());
        list.set_rows(sample_rows());
        list.apply_filter("web");
        assert_eq!(list.visible_count(), 2);
        assert_eq!(list.selected_id(), Some("srv-1"));
    }

    #[test]
    fn test_apply_filter_no_match() {
        let mut list = ResourceList::new(sample_columns());
        list.set_rows(sample_rows());
        list.apply_filter("nonexistent");
        assert_eq!(list.visible_count(), 0);
        assert!(list.selected_id().is_none());
    }

    #[test]
    fn test_clear_filter() {
        let mut list = ResourceList::new(sample_columns());
        list.set_rows(sample_rows());
        list.apply_filter("web");
        assert_eq!(list.visible_count(), 2);
        list.clear_filter();
        assert_eq!(list.visible_count(), 3);
    }

    #[test]
    fn test_apply_empty_filter_clears() {
        let mut list = ResourceList::new(sample_columns());
        list.set_rows(sample_rows());
        list.apply_filter("web");
        assert_eq!(list.visible_count(), 2);
        // Empty string should clear filter
        list.apply_filter("");
        assert_eq!(list.visible_count(), 3);
        assert!(list.search_term.is_none());
    }

    #[test]
    fn test_navigate_bounds_empty_list() {
        let mut list = ResourceList::new(sample_columns());
        // No rows set
        list.handle_key(key(KeyCode::Char('j')));
        assert_eq!(list.selected_index(), 0);
        assert!(list.selected_id().is_none());
    }

    #[test]
    fn test_row_style_active_uses_theme_done() {
        let style = RowStyleHint::Active.style();
        assert_eq!(style.fg, Some(Color::Green)); // Theme::done()
    }

    #[test]
    fn test_row_style_error_uses_theme_error() {
        let style = RowStyleHint::Error.style();
        assert_eq!(style.fg, Some(Color::Red)); // Theme::error()
    }

    #[test]
    fn test_selection_preserves_semantic_color() {
        // Theme::highlight() is Bold only (no fg) — patch preserves base fg
        let base = RowStyleHint::Active.style(); // Green
        let selected = base.patch(super::Theme::highlight());
        assert_eq!(selected.fg, Some(Color::Green)); // Green preserved
        assert!(selected.add_modifier.contains(Modifier::BOLD)); // Bold added
    }
}
