use std::collections::{HashMap, HashSet};

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::models::nova::Server;
use crate::ui::theme::Icons;

/// Status filter for the instance list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusFilter {
    All,
    Active,
    Error,
    Shutoff,
}

impl StatusFilter {
    pub fn cycle(self) -> Self {
        match self {
            Self::All => Self::Active,
            Self::Active => Self::Error,
            Self::Error => Self::Shutoff,
            Self::Shutoff => Self::All,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "ALL",
            Self::Active => "ACTIVE",
            Self::Error => "ERROR",
            Self::Shutoff => "SHUTOFF",
        }
    }

    fn matches(self, status: &str) -> bool {
        match self {
            Self::All => true,
            Self::Active => status == "ACTIVE",
            Self::Error => status == "ERROR",
            Self::Shutoff => status == "SHUTOFF" || status == "STOPPED",
        }
    }
}

pub struct InstanceList {
    servers: Vec<Server>,
    filtered_indices: Vec<usize>,
    selected: usize,
    checked: HashSet<String>,
    filter: StatusFilter,
    evac_status: HashMap<String, EvacInlineStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvacInlineStatus {
    InFlight,
    Success,
    Failed,
}

impl Default for InstanceList {
    fn default() -> Self { Self::new() }
}

impl InstanceList {
    pub fn new() -> Self {
        Self {
            servers: Vec::new(),
            filtered_indices: Vec::new(),
            selected: 0,
            checked: HashSet::new(),
            filter: StatusFilter::All,
            evac_status: HashMap::new(),
        }
    }

    pub fn set_servers(&mut self, servers: Vec<Server>) {
        self.servers = servers;
        self.checked.clear();
        self.evac_status.clear();
        self.rebuild_filter();
    }

    pub fn servers(&self) -> &[Server] {
        &self.servers
    }

    pub fn filter(&self) -> StatusFilter {
        self.filter
    }

    pub fn cycle_filter(&mut self) {
        self.filter = self.filter.cycle();
        self.rebuild_filter();
    }

    fn rebuild_filter(&mut self) {
        self.filtered_indices = self
            .servers
            .iter()
            .enumerate()
            .filter(|(_, s)| self.filter.matches(&s.status))
            .map(|(i, _)| i)
            .collect();
        if self.selected >= self.filtered_indices.len() && !self.filtered_indices.is_empty() {
            self.selected = self.filtered_indices.len() - 1;
        } else if self.filtered_indices.is_empty() {
            self.selected = 0;
        }
    }

    pub fn filtered_count(&self) -> usize {
        self.filtered_indices.len()
    }

    pub fn selected_server(&self) -> Option<&Server> {
        self.filtered_indices
            .get(self.selected)
            .and_then(|&i| self.servers.get(i))
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if !self.filtered_indices.is_empty() && self.selected < self.filtered_indices.len() - 1 {
            self.selected += 1;
        }
    }

    pub fn toggle_check(&mut self) {
        if let Some(server) = self.selected_server() {
            let id = server.id.clone();
            if !self.checked.remove(&id) {
                self.checked.insert(id);
            }
        }
    }

    pub fn select_all(&mut self) {
        for &i in &self.filtered_indices {
            if let Some(s) = self.servers.get(i) {
                self.checked.insert(s.id.clone());
            }
        }
    }

    pub fn deselect_all(&mut self) {
        self.checked.clear();
    }

    /// Returns checked IDs that are currently visible (in filtered_indices).
    /// Prevents phantom selection — hidden servers are never included.
    pub fn checked_ids(&self) -> Vec<String> {
        self.filtered_indices
            .iter()
            .filter_map(|&i| {
                let s = &self.servers[i];
                if self.checked.contains(&s.id) {
                    Some(s.id.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Count of checked items visible in current filter.
    pub fn checked_count(&self) -> usize {
        self.checked_ids().len()
    }

    pub fn is_checked(&self, id: &str) -> bool {
        self.checked.contains(id)
    }

    pub fn set_evac_status(&mut self, server_id: &str, status: EvacInlineStatus) {
        self.evac_status.insert(server_id.to_string(), status);
    }

    pub fn clear_evac_status(&mut self) {
        self.evac_status.clear();
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, focused: bool) {
        let border_style = if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let title = if focused {
            format!("[ Instances ({}) ]", self.filter.label())
        } else {
            format!("  Instances ({})  ", self.filter.label())
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if self.filtered_indices.is_empty() {
            let msg = Paragraph::new("No instances");
            frame.render_widget(msg, inner);
            return;
        }

        let visible = (inner.height as usize).max(1);
        let scroll_offset = if self.selected >= visible {
            self.selected - visible + 1
        } else {
            0
        };

        let mut lines: Vec<Line> = Vec::new();
        for (vi, &idx) in self.filtered_indices.iter().enumerate().skip(scroll_offset) {
            if lines.len() >= visible {
                break;
            }
            let server = &self.servers[idx];
            let is_selected = vi == self.selected;
            let checkbox = if self.is_checked(&server.id) { "☑" } else { "☐" };
            let icon = Icons::status_icon(&server.status);

            let name_style = if is_selected {
                Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default()
            };

            let mut spans = vec![
                Span::raw(format!("{checkbox} ")),
                Span::styled(icon, status_color(&server.status)),
                Span::raw(" "),
                Span::styled(&server.name, name_style),
            ];

            // Inline evacuate status
            if let Some(evac) = self.evac_status.get(&server.id) {
                let (label, style) = match evac {
                    EvacInlineStatus::InFlight => ("⟳", Style::default().fg(Color::Yellow)),
                    EvacInlineStatus::Success => ("✓", Style::default().fg(Color::Green)),
                    EvacInlineStatus::Failed => ("✗", Style::default().fg(Color::Red)),
                };
                spans.push(Span::raw(" "));
                spans.push(Span::styled(label, style));
            }

            lines.push(Line::from(spans));
        }

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }
}

fn status_color(status: &str) -> Style {
    match status {
        "ACTIVE" => Style::default().fg(Color::Green),
        "ERROR" => Style::default().fg(Color::Red),
        "BUILD" | "REBUILD" | "RESIZE" | "REBOOT" | "MIGRATING" | "VERIFY_RESIZE" => Style::default().fg(Color::Yellow),
        "SHUTOFF" | "STOPPED" => Style::default().fg(Color::DarkGray),
        _ => Style::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_server(id: &str, name: &str, status: &str) -> Server {
        Server {
            id: id.into(),
            name: name.into(),
            status: status.into(),
            tenant_id: Some("t1".into()),
            host: Some("compute-01".into()),
            host_id: None,
            availability_zone: Some("az1".into()),
            flavor: crate::models::nova::FlavorRef {
                id: "f1".into(),
                original_name: None,
                vcpus: None,
                ram: None,
                disk: None,
            },
            image: None,
            addresses: std::collections::HashMap::new(),
            created: "2026-01-01T00:00:00Z".into(),
            updated: None,
            key_name: None,
        }
    }

    #[test]
    fn test_instance_list_checkbox_toggle() {
        let mut list = InstanceList::new();
        list.set_servers(vec![
            make_server("s1", "web-01", "ACTIVE"),
            make_server("s2", "web-02", "ACTIVE"),
        ]);
        assert_eq!(list.checked_count(), 0);

        list.toggle_check(); // check s1
        assert!(list.is_checked("s1"));
        assert_eq!(list.checked_count(), 1);

        list.toggle_check(); // uncheck s1
        assert!(!list.is_checked("s1"));
        assert_eq!(list.checked_count(), 0);
    }

    #[test]
    fn test_instance_list_select_all() {
        let mut list = InstanceList::new();
        list.set_servers(vec![
            make_server("s1", "web-01", "ACTIVE"),
            make_server("s2", "web-02", "ERROR"),
            make_server("s3", "web-03", "SHUTOFF"),
        ]);
        list.select_all();
        assert_eq!(list.checked_count(), 3);

        list.deselect_all();
        assert_eq!(list.checked_count(), 0);
    }

    #[test]
    fn test_instance_list_status_filter_cycle() {
        let mut list = InstanceList::new();
        list.set_servers(vec![
            make_server("s1", "web-01", "ACTIVE"),
            make_server("s2", "web-02", "ERROR"),
            make_server("s3", "web-03", "SHUTOFF"),
        ]);
        assert_eq!(list.filtered_count(), 3); // ALL

        list.cycle_filter(); // → ACTIVE
        assert_eq!(list.filter(), StatusFilter::Active);
        assert_eq!(list.filtered_count(), 1);

        list.cycle_filter(); // → ERROR
        assert_eq!(list.filter(), StatusFilter::Error);
        assert_eq!(list.filtered_count(), 1);

        list.cycle_filter(); // → SHUTOFF
        assert_eq!(list.filter(), StatusFilter::Shutoff);
        assert_eq!(list.filtered_count(), 1);

        list.cycle_filter(); // → ALL
        assert_eq!(list.filter(), StatusFilter::All);
        assert_eq!(list.filtered_count(), 3);
    }

    #[test]
    fn test_instance_list_navigation() {
        let mut list = InstanceList::new();
        list.set_servers(vec![
            make_server("s1", "web-01", "ACTIVE"),
            make_server("s2", "web-02", "ACTIVE"),
        ]);
        assert_eq!(list.selected_server().unwrap().id, "s1");

        list.move_down();
        assert_eq!(list.selected_server().unwrap().id, "s2");

        list.move_down(); // at end
        assert_eq!(list.selected_server().unwrap().id, "s2");
    }

    #[test]
    fn test_instance_list_evac_inline_status() {
        let mut list = InstanceList::new();
        list.set_servers(vec![make_server("s1", "web-01", "ACTIVE")]);

        list.set_evac_status("s1", EvacInlineStatus::InFlight);
        assert_eq!(list.evac_status.get("s1"), Some(&EvacInlineStatus::InFlight));

        list.set_evac_status("s1", EvacInlineStatus::Success);
        assert_eq!(list.evac_status.get("s1"), Some(&EvacInlineStatus::Success));

        list.clear_evac_status();
        assert!(list.evac_status.is_empty());
    }

    #[test]
    fn test_status_filter_matches() {
        assert!(StatusFilter::All.matches("ACTIVE"));
        assert!(StatusFilter::All.matches("ERROR"));
        assert!(StatusFilter::Active.matches("ACTIVE"));
        assert!(!StatusFilter::Active.matches("ERROR"));
        assert!(StatusFilter::Error.matches("ERROR"));
        assert!(StatusFilter::Shutoff.matches("SHUTOFF"));
        assert!(StatusFilter::Shutoff.matches("STOPPED"));
        assert!(!StatusFilter::Shutoff.matches("ACTIVE"));
    }

    #[test]
    fn test_instance_list_empty_move_no_panic() {
        let mut list = InstanceList::new();
        list.move_down();
        list.move_up();
        assert!(list.selected_server().is_none());
    }

    #[test]
    fn test_phantom_selection_excluded_from_checked_ids() {
        let mut list = InstanceList::new();
        list.set_servers(vec![
            make_server("s1", "web-01", "ACTIVE"),
            make_server("s2", "web-02", "ERROR"),
        ]);
        // Check both servers in ALL filter
        list.select_all();
        assert_eq!(list.checked_count(), 2);

        // Switch to ACTIVE filter — s2 (ERROR) is hidden
        list.cycle_filter(); // → ACTIVE
        assert_eq!(list.filtered_count(), 1);

        // checked_ids should only return visible servers
        let ids = list.checked_ids();
        assert_eq!(ids.len(), 1);
        assert!(ids.contains(&"s1".to_string()));
        assert!(!ids.contains(&"s2".to_string())); // phantom excluded
    }

    #[test]
    fn test_set_servers_clears_checked_and_evac_status() {
        let mut list = InstanceList::new();
        list.set_servers(vec![make_server("s1", "web-01", "ACTIVE")]);
        list.toggle_check();
        list.set_evac_status("s1", EvacInlineStatus::Success);
        assert_eq!(list.checked_count(), 1);

        // Replace servers — should clear checked and evac_status
        list.set_servers(vec![make_server("s2", "db-01", "ACTIVE")]);
        assert_eq!(list.checked_count(), 0);
        assert!(list.evac_status.is_empty());
    }
}
