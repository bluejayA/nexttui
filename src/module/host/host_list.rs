use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::models::nova::Hypervisor;

pub struct HostList {
    hypervisors: Vec<Hypervisor>,
    selected: usize,
}

impl Default for HostList {
    fn default() -> Self { Self::new() }
}

impl HostList {
    pub fn new() -> Self {
        Self {
            hypervisors: Vec::new(),
            selected: 0,
        }
    }

    pub fn set_hypervisors(&mut self, hypervisors: Vec<Hypervisor>) {
        self.hypervisors = hypervisors;
        if self.selected >= self.hypervisors.len() && !self.hypervisors.is_empty() {
            self.selected = self.hypervisors.len() - 1;
        }
    }

    pub fn hypervisors(&self) -> &[Hypervisor] {
        &self.hypervisors
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn selected_hypervisor(&self) -> Option<&Hypervisor> {
        self.hypervisors.get(self.selected)
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if !self.hypervisors.is_empty() && self.selected < self.hypervisors.len() - 1 {
            self.selected += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.hypervisors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.hypervisors.is_empty()
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, focused: bool) {
        let border_style = if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let title = if focused { "[ Hosts ]" } else { "  Hosts  " };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if self.hypervisors.is_empty() {
            let msg = Paragraph::new("No hypervisors loaded");
            frame.render_widget(msg, inner);
            return;
        }

        // Each host takes 3 lines: hostname+status, resource bar, separator
        let visible_hosts = (inner.height as usize) / 3;
        let scroll_offset = if self.selected >= visible_hosts {
            self.selected - visible_hosts + 1
        } else {
            0
        };

        let mut lines: Vec<Line> = Vec::new();
        for (i, h) in self.hypervisors.iter().enumerate().skip(scroll_offset) {
            if lines.len() >= inner.height as usize {
                break;
            }
            let is_selected = i == self.selected;
            let host_icon = host_state_icon(&h.state, &h.status);
            let hostname_style = if is_selected {
                Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default()
            };

            // Line 1: icon + hostname + VM count
            lines.push(Line::from(vec![
                Span::styled(host_icon, host_state_style(&h.state, &h.status)),
                Span::raw(" "),
                Span::styled(&h.hypervisor_hostname, hostname_style),
                Span::raw(format!("  {} VMs", h.running_vms)),
            ]));

            // Line 2: resource usage summary
            if lines.len() < inner.height as usize {
                let cpu_pct = if h.vcpus > 0 { h.vcpus_used * 100 / h.vcpus } else { 0 };
                let mem_pct = if h.memory_mb > 0 { h.memory_mb_used * 100 / h.memory_mb } else { 0 };
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled("CPU", Style::default().fg(Color::DarkGray)),
                    Span::raw(format!(" {}/{}({cpu_pct}%) ", h.vcpus_used, h.vcpus)),
                    Span::styled("MEM", Style::default().fg(Color::DarkGray)),
                    Span::raw(format!(" {}/{}MB({mem_pct}%)", h.memory_mb_used, h.memory_mb)),
                ]));
            }

            // Line 3: separator
            if lines.len() < inner.height as usize {
                lines.push(Line::from(""));
            }
        }

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }
}

fn host_state_icon<'a>(state: &str, status: &str) -> &'a str {
    if status == "disabled" {
        return "⊘";
    }
    match state {
        "up" => "●",
        "down" => "✗",
        _ => "?",
    }
}

fn host_state_style(state: &str, status: &str) -> Style {
    if status == "disabled" {
        return Style::default().fg(Color::DarkGray);
    }
    match state {
        "up" => Style::default().fg(Color::Green),
        "down" => Style::default().fg(Color::Red),
        _ => Style::default().fg(Color::Yellow),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_hypervisor(hostname: &str, vcpus: u32, vcpus_used: u32, state: &str, status: &str) -> Hypervisor {
        Hypervisor {
            id: "1".into(),
            hypervisor_hostname: hostname.into(),
            hypervisor_type: "QEMU".into(),
            vcpus,
            vcpus_used,
            memory_mb: 32768,
            memory_mb_used: 16384,
            local_gb: 500,
            local_gb_used: 200,
            running_vms: 5,
            status: status.into(),
            state: state.into(),
        }
    }

    #[test]
    fn test_host_list_selection_movement() {
        let mut list = HostList::new();
        list.set_hypervisors(vec![
            make_hypervisor("compute-01", 16, 8, "up", "enabled"),
            make_hypervisor("compute-02", 32, 0, "up", "enabled"),
            make_hypervisor("compute-03", 16, 16, "down", "disabled"),
        ]);
        assert_eq!(list.selected_index(), 0);

        list.move_down();
        assert_eq!(list.selected_index(), 1);

        list.move_down();
        assert_eq!(list.selected_index(), 2);

        list.move_down(); // at end, should stay
        assert_eq!(list.selected_index(), 2);

        list.move_up();
        assert_eq!(list.selected_index(), 1);

        list.move_up();
        assert_eq!(list.selected_index(), 0);

        list.move_up(); // at start, should stay
        assert_eq!(list.selected_index(), 0);
    }

    #[test]
    fn test_host_list_selected_hypervisor() {
        let mut list = HostList::new();
        assert!(list.selected_hypervisor().is_none());

        list.set_hypervisors(vec![
            make_hypervisor("compute-01", 16, 8, "up", "enabled"),
        ]);
        let h = list.selected_hypervisor().unwrap();
        assert_eq!(h.hypervisor_hostname, "compute-01");
    }

    #[test]
    fn test_host_list_set_hypervisors_clamps_selection() {
        let mut list = HostList::new();
        list.set_hypervisors(vec![
            make_hypervisor("a", 1, 0, "up", "enabled"),
            make_hypervisor("b", 1, 0, "up", "enabled"),
            make_hypervisor("c", 1, 0, "up", "enabled"),
        ]);
        list.move_down();
        list.move_down();
        assert_eq!(list.selected_index(), 2);

        // Shrink list — selection should clamp
        list.set_hypervisors(vec![
            make_hypervisor("a", 1, 0, "up", "enabled"),
        ]);
        assert_eq!(list.selected_index(), 0);
    }

    #[test]
    fn test_host_list_empty() {
        let list = HostList::new();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
        assert!(list.selected_hypervisor().is_none());
    }

    #[test]
    fn test_host_list_empty_move_no_panic() {
        let mut list = HostList::new();
        list.move_down(); // should not panic on empty
        list.move_up();
        assert_eq!(list.selected_index(), 0);
    }

    #[test]
    fn test_host_state_icons() {
        assert_eq!(host_state_icon("up", "enabled"), "●");
        assert_eq!(host_state_icon("down", "enabled"), "✗");
        assert_eq!(host_state_icon("up", "disabled"), "⊘");
        assert_eq!(host_state_icon("unknown", "enabled"), "?");
    }
}
