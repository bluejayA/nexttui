use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Cell, Paragraph, Row as TuiRow, Table};

use crate::action::Action;
use crate::component::Component;
use crate::context::ActionSender;
use crate::event::AppEvent;
use crate::models::keystone::Project;
use crate::models::nova::Hypervisor;
use crate::port::types::TenantUsage;
use crate::ui::gauge_bar::GaugeBar;
use crate::ui::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateRangePreset {
    ThisMonth,
    LastMonth,
    Last7Days,
}

impl DateRangePreset {
    pub fn start_end(&self) -> (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) {
        use chrono::{Datelike, TimeZone, Utc};

        let now = chrono::Local::now().with_timezone(&Utc);
        match self {
            DateRangePreset::ThisMonth => {
                let start = Utc
                    .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
                    .single()
                    .unwrap_or(now);
                (start, now)
            }
            DateRangePreset::LastMonth => {
                // First day of current month
                let this_month_start = Utc
                    .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
                    .single()
                    .unwrap_or(now);
                // Last month start: subtract one month
                let (y, m) = if now.month() == 1 {
                    (now.year() - 1, 12)
                } else {
                    (now.year(), now.month() - 1)
                };
                let last_month_start = Utc
                    .with_ymd_and_hms(y, m, 1, 0, 0, 0)
                    .single()
                    .unwrap_or(now);
                (last_month_start, this_month_start)
            }
            DateRangePreset::Last7Days => {
                let start = now - chrono::Duration::days(7);
                (start, now)
            }
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            DateRangePreset::ThisMonth => "This Month",
            DateRangePreset::LastMonth => "Last Month",
            DateRangePreset::Last7Days => "Last 7 Days",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            DateRangePreset::ThisMonth => DateRangePreset::LastMonth,
            DateRangePreset::LastMonth => DateRangePreset::Last7Days,
            DateRangePreset::Last7Days => DateRangePreset::ThisMonth,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            DateRangePreset::ThisMonth => DateRangePreset::Last7Days,
            DateRangePreset::LastMonth => DateRangePreset::ThisMonth,
            DateRangePreset::Last7Days => DateRangePreset::LastMonth,
        }
    }
}

pub struct UsageModule {
    tenant_usages: Vec<TenantUsage>,
    hypervisors: Vec<Hypervisor>,
    cached_projects: Vec<Project>,
    date_range: DateRangePreset,
    loading: bool,
    mounted: bool,
    error_message: Option<String>,
    scroll_offset: usize,
    action_tx: ActionSender,
}

impl UsageModule {
    pub fn new(action_tx: ActionSender) -> Self {
        Self {
            tenant_usages: Vec::new(),
            hypervisors: Vec::new(),
            cached_projects: Vec::new(),
            date_range: DateRangePreset::ThisMonth,
            loading: false,
            mounted: false,
            error_message: None,
            scroll_offset: 0,
            action_tx,
        }
    }

    pub fn date_range(&self) -> DateRangePreset {
        self.date_range
    }

    pub fn tenant_usages(&self) -> &[TenantUsage] {
        &self.tenant_usages
    }

    pub fn hypervisors(&self) -> &[Hypervisor] {
        &self.hypervisors
    }

    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    fn fetch_usage(&self) {
        let (start, end) = self.date_range.start_end();
        let _ = self.action_tx.send(Action::FetchUsage {
            start: start.to_rfc3339(),
            end: end.to_rfc3339(),
        });
    }

    fn resolve_project_name(&self, tenant_id: &str) -> String {
        self.cached_projects
            .iter()
            .find(|p| p.id == tenant_id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| {
                if tenant_id.len() > 8 {
                    format!("{}...", &tenant_id[..8])
                } else {
                    tenant_id.to_string()
                }
            })
    }

    fn sorted_usages(&self) -> Vec<&TenantUsage> {
        let mut usages: Vec<&TenantUsage> = self.tenant_usages.iter().collect();
        usages.sort_by(|a, b| {
            b.total_vcpus_usage
                .partial_cmp(&a.total_vcpus_usage)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        usages
    }

    fn render_infra_summary(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Infrastructure Summary ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Theme::focus_border());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if self.hypervisors.is_empty() {
            let msg = Paragraph::new("No hypervisor data available");
            frame.render_widget(msg, inner);
            return;
        }

        let mut total_vcpus: u64 = 0;
        let mut used_vcpus: u64 = 0;
        let mut total_ram: u64 = 0;
        let mut used_ram: u64 = 0;
        let mut total_disk: u64 = 0;
        let mut used_disk: u64 = 0;
        let mut total_vms: u64 = 0;

        for hv in &self.hypervisors {
            total_vcpus += u64::from(hv.vcpus);
            used_vcpus += u64::from(hv.vcpus_used);
            total_ram += u64::from(hv.memory_mb);
            used_ram += u64::from(hv.memory_mb_used);
            total_disk += u64::from(hv.local_gb);
            used_disk += u64::from(hv.local_gb_used);
            total_vms += u64::from(hv.running_vms);
        }

        // 4x1 grid: vCPU | RAM | Disk | VMs side by side
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ])
            .split(inner);

        self.render_mini_gauge(frame, cols[0], "vCPU", used_vcpus, total_vcpus, "");
        self.render_mini_gauge(
            frame,
            cols[1],
            "RAM",
            used_ram / 1024,
            total_ram / 1024,
            "GB",
        );
        self.render_mini_gauge(frame, cols[2], "Disk", used_disk, total_disk, "GB");
        self.render_mini_gauge(frame, cols[3], "VMs", total_vms, total_vms.max(1), "");
    }

    fn render_mini_gauge(
        &self,
        frame: &mut Frame,
        area: Rect,
        title: &str,
        used: u64,
        total: u64,
        unit: &str,
    ) {
        let pct = if total > 0 {
            (used as f64 / total as f64 * 100.0) as u16
        } else {
            0
        };
        let color = match pct {
            0..=70 => Color::Green,
            71..=90 => Color::Yellow,
            _ => Color::Red,
        };

        let mini_block = Block::default()
            .title(format!(" {title} "))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(color));

        let mini_inner = mini_block.inner(area);
        frame.render_widget(mini_block, area);

        if mini_inner.height < 3 || mini_inner.width < 6 {
            return;
        }

        let unit_str = if unit.is_empty() {
            String::new()
        } else {
            format!(" {unit}")
        };
        let info_line = Line::from(vec![
            Span::styled(
                format!(" Used: {used}{unit_str}"),
                Style::default().fg(Color::White),
            ),
            Span::styled(
                format!("  Total: {total}{unit_str}"),
                Style::default().fg(Color::DarkGray),
            ),
        ]);

        let bar_width = mini_inner.width.saturating_sub(2);
        let gauge = GaugeBar::new("", used, total).with_bar_width(bar_width);
        let bar_line = gauge.render_line();
        // Remove the label span (first span is empty label)
        let bar_spans: Vec<Span> = bar_line.spans.into_iter().skip(1).collect();
        let bar_only = Line::from(bar_spans);

        let pct_line = Line::from(Span::styled(
            format!(" {pct}%"),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ));

        let paragraph = Paragraph::new(vec![info_line, bar_only, pct_line]);
        frame.render_widget(paragraph, mini_inner);
    }

    fn render_project_usage(&self, frame: &mut Frame, area: Rect) {
        let title = format!(
            " Project Usage \u{25C0} {} \u{25B6} ",
            self.date_range.label()
        );
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Theme::focus_border());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if self.loading {
            let msg = Paragraph::new("Loading...");
            frame.render_widget(msg, inner);
            return;
        }

        if let Some(ref err) = self.error_message {
            let msg = Paragraph::new(err.as_str()).style(Theme::error());
            frame.render_widget(msg, inner);
            return;
        }

        if self.tenant_usages.is_empty() {
            let msg = Paragraph::new("No usage data");
            frame.render_widget(msg, inner);
            return;
        }

        let header_cells = ["Project", "vCPU-h", "RAM MB-h", "Disk GB-h", "Instances"]
            .iter()
            .map(|h| {
                Cell::from(*h).style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
            });
        let header = TuiRow::new(header_cells).height(1);

        let usages = self.sorted_usages();
        let visible_usages: Vec<&&TenantUsage> = usages.iter().skip(self.scroll_offset).collect();

        let rows: Vec<TuiRow> = visible_usages
            .iter()
            .map(|u| {
                let name = self.resolve_project_name(&u.tenant_id);
                let cells = vec![
                    Cell::from(name),
                    Cell::from(format!("{:.1}", u.total_vcpus_usage)),
                    Cell::from(format!("{:.1}", u.total_memory_mb_usage)),
                    Cell::from(format!("{:.1}", u.total_local_gb_usage)),
                    Cell::from(format!("{}", u.server_usages.len())),
                ];
                TuiRow::new(cells)
            })
            .collect();

        let widths = [
            Constraint::Min(20),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(10),
        ];

        let table = Table::new(rows, widths)
            .header(header)
            .row_highlight_style(Theme::highlight());

        frame.render_widget(table, inner);
    }

    fn render_hypervisor_load(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Hypervisor Allocation ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Theme::focus_border());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if self.hypervisors.is_empty() {
            let msg = Paragraph::new("No hypervisor data");
            frame.render_widget(msg, inner);
            return;
        }

        // B-2 layout: left = host list with gauges, right = summary panel
        let lr = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(inner);

        // --- Left: host list ---
        let bar_w = lr[0].width.saturating_sub(4) / 2; // split remaining between vCPU and RAM bars
        let bar_w = bar_w.min(15);

        let mut total_vcpus: u64 = 0;
        let mut used_vcpus: u64 = 0;
        let mut total_ram_gb: u64 = 0;
        let mut used_ram_gb: u64 = 0;
        let mut hosts_up: u32 = 0;
        let mut hosts_down: u32 = 0;
        let mut high_cpu_hosts: Vec<String> = Vec::new();

        // Dynamic hostname padding based on longest hostname
        let max_hostname_len = self
            .hypervisors
            .iter()
            .map(|hv| hv.hypervisor_hostname.len())
            .max()
            .unwrap_or(10);

        let lines: Vec<Line> = self
            .hypervisors
            .iter()
            .map(|hv| {
                let vcpu_u = u64::from(hv.vcpus_used);
                let vcpu_t = u64::from(hv.vcpus);
                let ram_u = u64::from(hv.memory_mb_used) / 1024;
                let ram_t = u64::from(hv.memory_mb) / 1024;
                let vms = hv.running_vms;

                total_vcpus += vcpu_t;
                used_vcpus += vcpu_u;
                total_ram_gb += ram_t;
                used_ram_gb += ram_u;

                let is_up = hv.state == "up" && hv.status == "enabled";
                if is_up {
                    hosts_up += 1;
                } else {
                    hosts_down += 1;
                }

                let cpu_pct = if vcpu_t > 0 {
                    (vcpu_u as f64 / vcpu_t as f64 * 100.0) as u16
                } else {
                    0
                };
                if cpu_pct > 90 {
                    high_cpu_hosts.push(hv.hypervisor_hostname.clone());
                }

                let cpu_gauge = GaugeBar::new("", vcpu_u, vcpu_t).with_bar_width(bar_w);
                let ram_gauge = GaugeBar::new("", ram_u, ram_t).with_bar_width(bar_w);
                let cpu_color = cpu_gauge.color();
                let ram_color = ram_gauge.color();

                let state_icon = if is_up { "●" } else { "✗" };
                let state_color = if is_up { Color::Green } else { Color::Red };

                let hostname = &hv.hypervisor_hostname;

                let cpu_filled = ((bar_w as f64) * cpu_gauge.ratio()) as u16;
                let cpu_empty = bar_w.saturating_sub(cpu_filled);
                let ram_filled = ((bar_w as f64) * ram_gauge.ratio()) as u16;
                let ram_empty = bar_w.saturating_sub(ram_filled);

                Line::from(vec![
                    Span::styled(format!(" {state_icon} "), Style::default().fg(state_color)),
                    Span::styled(
                        format!("{:<width$}", hostname, width = max_hostname_len),
                        Style::default().fg(Color::White),
                    ),
                    Span::raw(" vCPU["),
                    Span::styled(
                        "▓".repeat(cpu_filled as usize),
                        Style::default().fg(cpu_color),
                    ),
                    Span::styled(
                        "·".repeat(cpu_empty as usize),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::raw("]"),
                    Span::styled(
                        format!("{:>3}/{:<3}", vcpu_u, vcpu_t),
                        Style::default().fg(Color::White),
                    ),
                    Span::raw(" RAM["),
                    Span::styled(
                        "▓".repeat(ram_filled as usize),
                        Style::default().fg(ram_color),
                    ),
                    Span::styled(
                        "·".repeat(ram_empty as usize),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::raw("]"),
                    Span::styled(format!("{:>3}G", ram_u), Style::default().fg(Color::White)),
                    Span::styled(format!(" VMs:{vms}"), Style::default().fg(Color::DarkGray)),
                ])
            })
            .collect();

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, lr[0]);

        // --- Right: summary panel ---
        let summary_block = Block::default()
            .title(" Summary ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray));

        let summary_inner = summary_block.inner(lr[1]);
        frame.render_widget(summary_block, lr[1]);

        let total_hosts = hosts_up + hosts_down;
        let cpu_pct = if total_vcpus > 0 {
            (used_vcpus as f64 / total_vcpus as f64 * 100.0) as u16
        } else {
            0
        };
        let ram_pct = if total_ram_gb > 0 {
            (used_ram_gb as f64 / total_ram_gb as f64 * 100.0) as u16
        } else {
            0
        };

        let mut summary_lines = vec![
            Line::from(vec![
                Span::styled(" Hosts: ", Style::default().fg(Color::White)),
                Span::styled(
                    format!("{total_hosts}"),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" ({hosts_up} up"),
                    Style::default().fg(Color::Green),
                ),
                if hosts_down > 0 {
                    Span::styled(
                        format!(", {hosts_down} down"),
                        Style::default().fg(Color::Red),
                    )
                } else {
                    Span::raw("")
                },
                Span::raw(")"),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(" Total vCPU: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{used_vcpus}/{total_vcpus} ({cpu_pct}%)"),
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                Span::styled(" Total RAM:  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{used_ram_gb}/{total_ram_gb} GB ({ram_pct}%)"),
                    Style::default().fg(Color::White),
                ),
            ]),
        ];

        if !high_cpu_hosts.is_empty() {
            summary_lines.push(Line::from(""));
            summary_lines.push(Line::from(Span::styled(
                format!(" ⚠ {} hosts > 90% vCPU", high_cpu_hosts.len()),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )));
            for h in high_cpu_hosts.iter().take(5) {
                summary_lines.push(Line::from(Span::styled(
                    format!("   {h}"),
                    Style::default().fg(Color::Red),
                )));
            }
        }

        let summary_paragraph = Paragraph::new(summary_lines);
        frame.render_widget(summary_paragraph, summary_inner);
    }
}

impl Component for UsageModule {
    fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        if !self.mounted {
            self.mounted = true;
            self.loading = true;
            self.fetch_usage();
            let _ = self.action_tx.send(Action::FetchHypervisors);
            let _ = self.action_tx.send(Action::FetchProjects);
            // Consume the first key to avoid stale fetch race
            return None;
        }

        match key.code {
            KeyCode::Left => Some(Action::FocusSidebar),
            KeyCode::Char('[') => {
                // '[' for previous period (avoids 'h' conflict with Host Ops)
                self.date_range = self.date_range.prev();
                self.scroll_offset = 0;
                self.loading = true;
                self.fetch_usage();
                None
            }
            KeyCode::Char(']') => {
                // ']' for next period
                self.date_range = self.date_range.next();
                self.scroll_offset = 0;
                self.loading = true;
                self.fetch_usage();
                None
            }
            KeyCode::Char('j') | KeyCode::Down => {
                let max_offset = self.tenant_usages.len().saturating_sub(1);
                if self.scroll_offset < max_offset {
                    self.scroll_offset = self.scroll_offset.saturating_add(1);
                }
                None
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
                None
            }
            KeyCode::Char('r') => {
                self.loading = true;
                self.fetch_usage();
                None
            }
            KeyCode::Esc => Some(Action::Back),
            _ => None,
        }
    }

    fn on_context_changed(&mut self) {
        self.tenant_usages.clear();
        self.hypervisors.clear();
        self.cached_projects.clear();
        self.loading = true;
        self.error_message = None;
        // Codex review 2차 P2: hypervisors/projects are not covered by the
        // single-valued `refresh_action` (which returns FetchUsage), so they
        // must be re-fetched explicitly here or the screen stays blank until
        // the user presses `r`.
        let _ = self.action_tx.send(Action::FetchHypervisors);
        let _ = self.action_tx.send(Action::FetchProjects);
    }

    fn handle_event(&mut self, event: &AppEvent) {
        match event {
            AppEvent::UsageLoaded(usages) => {
                self.tenant_usages = usages.clone();
                self.loading = false;
                self.error_message = None;
            }
            AppEvent::HypervisorsLoaded(hvs) => {
                self.hypervisors = hvs.clone();
            }
            AppEvent::ProjectsLoaded(projs) => {
                self.cached_projects = projs.clone();
            }
            AppEvent::ApiError {
                operation, message, ..
            } => {
                self.error_message = Some(format!("{operation}: {message}"));
                self.loading = false;
            }
            _ => {}
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        let remaining = area.height.saturating_sub(7); // after Infrastructure Summary
        let half = remaining / 2;
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(7),    // Infrastructure Summary (fixed)
                Constraint::Length(half), // Project Usage (50% of remaining)
                Constraint::Min(half),    // Hypervisor Allocation (50% of remaining)
            ])
            .split(area);

        self.render_infra_summary(frame, chunks[0]);
        self.render_project_usage(frame, chunks[1]);
        self.render_hypervisor_load(frame, chunks[2]);
    }

    fn help_hint(&self) -> &str {
        "[/]:Period  j/k:Scroll  r:Refresh"
    }

    fn refresh_action(&self) -> Option<Action> {
        // Codex review 2차 P2: returning the FetchUsage action lets App's
        // ContextChanged arm dispatch it automatically after a project/cloud
        // switch, so the usage screen does not stay empty until `r`.
        let (start, end) = self.date_range.start_end();
        Some(Action::FetchUsage {
            start: start.to_rfc3339(),
            end: end.to_rfc3339(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::test_action_channel;
    use crate::port::types::ServerUsageEntry;
    use chrono::Datelike;
    use crossterm::event::KeyEvent;

    fn make_tx() -> ActionSender {
        let (tx, _rx) = test_action_channel();
        tx
    }

    fn make_module() -> UsageModule {
        UsageModule::new(make_tx())
    }

    /// Creates a module that has already consumed the mount key,
    /// so subsequent handle_key calls behave normally.
    fn make_mounted_module() -> UsageModule {
        let (tx, mut rx) = test_action_channel();
        let mut m = UsageModule::new(tx);
        // Trigger mount with a dummy key (consumed, returns None)
        m.handle_key(key(KeyCode::Char('x')));
        // Drain FetchUsage, FetchHypervisors, FetchProjects
        while rx.try_recv().is_ok() {}
        m
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::from(code)
    }

    fn sample_hypervisors() -> Vec<Hypervisor> {
        vec![
            Hypervisor {
                id: "1".into(),
                hypervisor_hostname: "compute-01".into(),
                hypervisor_type: "QEMU".into(),
                vcpus: 64,
                vcpus_used: 32,
                memory_mb: 131072,
                memory_mb_used: 65536,
                local_gb: 2000,
                local_gb_used: 500,
                running_vms: 20,
                status: "enabled".into(),
                state: "up".into(),
            },
            Hypervisor {
                id: "2".into(),
                hypervisor_hostname: "compute-02".into(),
                hypervisor_type: "QEMU".into(),
                vcpus: 64,
                vcpus_used: 48,
                memory_mb: 131072,
                memory_mb_used: 98304,
                local_gb: 2000,
                local_gb_used: 1500,
                running_vms: 30,
                status: "enabled".into(),
                state: "up".into(),
            },
        ]
    }

    fn sample_usages() -> Vec<TenantUsage> {
        vec![
            TenantUsage {
                tenant_id: "proj-1".into(),
                total_vcpus_usage: 100.0,
                total_memory_mb_usage: 204800.0,
                total_local_gb_usage: 500.0,
                total_hours: 720.0,
                server_usages: vec![ServerUsageEntry {
                    instance_id: "i1".into(),
                    name: "web".into(),
                    hours: 360.0,
                    vcpus: 2,
                    memory_mb: 4096,
                    local_gb: 50,
                    state: "active".into(),
                }],
            },
            TenantUsage {
                tenant_id: "proj-2".into(),
                total_vcpus_usage: 200.0,
                total_memory_mb_usage: 409600.0,
                total_local_gb_usage: 1000.0,
                total_hours: 720.0,
                server_usages: vec![
                    ServerUsageEntry {
                        instance_id: "i2".into(),
                        name: "db".into(),
                        hours: 720.0,
                        vcpus: 4,
                        memory_mb: 8192,
                        local_gb: 100,
                        state: "active".into(),
                    },
                    ServerUsageEntry {
                        instance_id: "i3".into(),
                        name: "cache".into(),
                        hours: 360.0,
                        vcpus: 2,
                        memory_mb: 4096,
                        local_gb: 50,
                        state: "active".into(),
                    },
                ],
            },
        ]
    }

    fn sample_projects() -> Vec<Project> {
        vec![
            Project {
                id: "proj-1".into(),
                name: "web-team".into(),
                description: None,
                enabled: true,
                domain_id: Some("default".into()),
            },
            Project {
                id: "proj-2".into(),
                name: "data-team".into(),
                description: None,
                enabled: true,
                domain_id: Some("default".into()),
            },
        ]
    }

    // === DateRangePreset tests ===

    #[test]
    fn test_date_range_label() {
        assert_eq!(DateRangePreset::ThisMonth.label(), "This Month");
        assert_eq!(DateRangePreset::LastMonth.label(), "Last Month");
        assert_eq!(DateRangePreset::Last7Days.label(), "Last 7 Days");
    }

    #[test]
    fn test_date_range_next_cycles() {
        let mut d = DateRangePreset::ThisMonth;
        d = d.next();
        assert_eq!(d, DateRangePreset::LastMonth);
        d = d.next();
        assert_eq!(d, DateRangePreset::Last7Days);
        d = d.next();
        assert_eq!(d, DateRangePreset::ThisMonth);
    }

    #[test]
    fn test_date_range_prev_cycles() {
        let mut d = DateRangePreset::ThisMonth;
        d = d.prev();
        assert_eq!(d, DateRangePreset::Last7Days);
        d = d.prev();
        assert_eq!(d, DateRangePreset::LastMonth);
        d = d.prev();
        assert_eq!(d, DateRangePreset::ThisMonth);
    }

    #[test]
    fn test_date_range_start_end_this_month() {
        let (start, end) = DateRangePreset::ThisMonth.start_end();
        assert!(start < end);
        assert_eq!(start.day(), 1);
    }

    #[test]
    fn test_date_range_start_end_last_month() {
        let (start, end) = DateRangePreset::LastMonth.start_end();
        assert!(start < end);
        assert_eq!(start.day(), 1);
        assert_eq!(end.day(), 1);
    }

    #[test]
    fn test_date_range_start_end_last_7_days() {
        let (start, end) = DateRangePreset::Last7Days.start_end();
        assert!(start < end);
        let diff = end - start;
        // Should be roughly 7 days (within a small margin)
        assert!(diff.num_days() >= 6 && diff.num_days() <= 7);
    }

    // === UsageModule construction ===

    #[test]
    fn test_new_module_defaults() {
        let m = make_module();
        assert_eq!(m.date_range(), DateRangePreset::ThisMonth);
        assert!(m.tenant_usages().is_empty());
        assert!(m.hypervisors().is_empty());
        assert_eq!(m.scroll_offset(), 0);
        assert!(m.error_message().is_none());
    }

    // === handle_event tests ===

    #[test]
    fn test_handle_event_usage_loaded() {
        let mut m = make_module();
        m.loading = true;
        let usages = sample_usages();
        m.handle_event(&AppEvent::UsageLoaded(usages.clone()));
        assert_eq!(m.tenant_usages().len(), 2);
        assert!(!m.loading);
        assert!(m.error_message().is_none());
    }

    #[test]
    fn test_handle_event_hypervisors_loaded() {
        let mut m = make_module();
        let hvs = sample_hypervisors();
        m.handle_event(&AppEvent::HypervisorsLoaded(hvs.clone()));
        assert_eq!(m.hypervisors().len(), 2);
    }

    #[test]
    fn test_handle_event_projects_loaded() {
        let mut m = make_module();
        let projs = sample_projects();
        m.handle_event(&AppEvent::ProjectsLoaded(projs));
        assert_eq!(m.cached_projects.len(), 2);
    }

    #[test]
    fn test_handle_event_api_error() {
        let mut m = make_module();
        m.loading = true;
        m.handle_event(&AppEvent::ApiError {
            operation: "FetchUsage".into(),
            message: "timeout".into(),
        });
        assert!(!m.loading);
        assert!(m.error_message().is_some());
        assert!(m.error_message().is_some_and(|e| e.contains("timeout")));
    }

    // === handle_key tests ===

    #[test]
    fn test_key_bracket_changes_date_range() {
        let (tx, mut rx) = test_action_channel();
        let mut m = UsageModule::new(tx);
        assert_eq!(m.date_range(), DateRangePreset::ThisMonth);

        // First key triggers mount (consumed)
        m.handle_key(key(KeyCode::Char('r')));
        // Drain mount fetch actions
        while rx.try_recv().is_ok() {}

        // Now '[' changes date range
        let result = m.handle_key(key(KeyCode::Char('[')));
        assert!(result.is_none());
        assert_eq!(m.date_range(), DateRangePreset::Last7Days);
        assert!(m.loading);

        let sent = rx.try_recv();
        assert!(sent.is_ok());
        match sent {
            Ok(Action::FetchUsage { .. }) => {}
            other => panic!("expected FetchUsage, got {other:?}"),
        }
    }

    #[test]
    fn test_key_bracket_right_changes_date_range() {
        let (tx, mut rx) = test_action_channel();
        let mut m = UsageModule::new(tx);

        // Mount
        m.handle_key(key(KeyCode::Char('r')));
        while rx.try_recv().is_ok() {}

        let result = m.handle_key(key(KeyCode::Char(']')));
        assert!(result.is_none());
        assert_eq!(m.date_range(), DateRangePreset::LastMonth);
        assert!(m.loading);

        let sent = rx.try_recv();
        assert!(matches!(sent, Ok(Action::FetchUsage { .. })));
    }

    #[test]
    fn test_key_j_scrolls_down() {
        let mut m = make_mounted_module();
        // Need usages to allow scrolling past 0
        m.tenant_usages = sample_usages();
        assert_eq!(m.scroll_offset(), 0);
        m.handle_key(key(KeyCode::Char('j')));
        assert_eq!(m.scroll_offset(), 1);
    }

    #[test]
    fn test_key_k_scrolls_up() {
        let mut m = make_mounted_module();
        m.scroll_offset = 3;
        m.handle_key(key(KeyCode::Char('k')));
        assert_eq!(m.scroll_offset(), 2);
    }

    #[test]
    fn test_key_k_saturates_at_zero() {
        let mut m = make_module();
        m.handle_key(key(KeyCode::Char('k')));
        assert_eq!(m.scroll_offset(), 0);
    }

    #[test]
    fn test_key_r_refreshes() {
        let (tx, mut rx) = test_action_channel();
        let mut m = UsageModule::new(tx);

        let result = m.handle_key(key(KeyCode::Char('r')));
        assert!(result.is_none());
        assert!(m.loading);

        let sent = rx.try_recv();
        assert!(matches!(sent, Ok(Action::FetchUsage { .. })));
    }

    #[test]
    fn test_key_left_focuses_sidebar() {
        let mut m = make_mounted_module();
        let result = m.handle_key(key(KeyCode::Left));
        assert!(matches!(result, Some(Action::FocusSidebar)));
    }

    #[test]
    fn test_key_esc_returns_back() {
        let mut m = make_mounted_module();
        let result = m.handle_key(key(KeyCode::Esc));
        assert!(matches!(result, Some(Action::Back)));
    }

    #[test]
    fn test_key_down_scrolls() {
        let mut m = make_mounted_module();
        m.tenant_usages = sample_usages();
        m.handle_key(key(KeyCode::Down));
        assert_eq!(m.scroll_offset(), 1);
    }

    #[test]
    fn test_key_up_scrolls() {
        let mut m = make_mounted_module();
        m.scroll_offset = 5;
        m.handle_key(key(KeyCode::Up));
        assert_eq!(m.scroll_offset(), 4);
    }

    #[test]
    fn test_unknown_key_returns_none() {
        let mut m = make_module();
        let result = m.handle_key(key(KeyCode::Char('z')));
        assert!(result.is_none());
    }

    // === help_hint ===

    #[test]
    fn test_help_hint() {
        let m = make_module();
        assert_eq!(m.help_hint(), "[/]:Period  j/k:Scroll  r:Refresh");
    }

    // === refresh_action ===

    #[test]
    fn test_refresh_action_returns_fetch_usage() {
        // Codex review 2차 P2: after context switch, App drains
        // refresh_action() so the new project's usage data starts loading
        // immediately rather than leaving the screen empty until `r`.
        let m = make_module();
        match m.refresh_action() {
            Some(Action::FetchUsage { .. }) => {}
            other => panic!("expected Some(FetchUsage), got {other:?}"),
        }
    }

    #[test]
    fn test_on_context_changed_dispatches_hypervisors_and_projects() {
        // Codex review 2차 P2: on_context_changed clears hypervisors and
        // cached_projects, so it must also re-dispatch their fetches.
        // (FetchUsage is handled by refresh_action via the App loop.)
        let (tx, mut rx) = test_action_channel();
        let mut m = UsageModule::new(tx);
        while rx.try_recv().is_ok() {}

        m.on_context_changed();

        let mut saw_hv = false;
        let mut saw_proj = false;
        while let Ok(action) = rx.try_recv() {
            match action {
                Action::FetchHypervisors => saw_hv = true,
                Action::FetchProjects => saw_proj = true,
                _ => {}
            }
        }
        assert!(saw_hv, "on_context_changed must dispatch FetchHypervisors");
        assert!(saw_proj, "on_context_changed must dispatch FetchProjects");
    }

    // === resolve_project_name ===

    #[test]
    fn test_resolve_project_name_found() {
        let mut m = make_module();
        m.cached_projects = sample_projects();
        assert_eq!(m.resolve_project_name("proj-1"), "web-team");
    }

    #[test]
    fn test_resolve_project_name_not_found_short_id() {
        let m = make_module();
        assert_eq!(m.resolve_project_name("abc"), "abc");
    }

    #[test]
    fn test_resolve_project_name_not_found_long_id() {
        let m = make_module();
        let long_id = "abcdefghij1234567890";
        let resolved = m.resolve_project_name(long_id);
        assert_eq!(resolved, "abcdefgh...");
    }

    // === sorted_usages ===

    #[test]
    fn test_sorted_usages_by_vcpu_desc() {
        let mut m = make_module();
        m.tenant_usages = sample_usages();
        let sorted = m.sorted_usages();
        assert_eq!(sorted.len(), 2);
        // proj-2 has 200 vCPU-h, proj-1 has 100 vCPU-h
        assert_eq!(sorted[0].tenant_id, "proj-2");
        assert_eq!(sorted[1].tenant_id, "proj-1");
    }

    // === Component trait: unrelated events are no-op ===

    #[test]
    fn test_handle_event_unrelated_is_noop() {
        let mut m = make_module();
        m.handle_event(&AppEvent::ServersLoaded(vec![]));
        assert!(m.tenant_usages().is_empty());
        assert!(m.hypervisors().is_empty());
    }

    // === Date range transitions + fetch ===

    #[test]
    fn test_bracket_left_cycle_returns_to_original() {
        let mut m = make_mounted_module();
        // '[' → Last7Days, '[' → LastMonth, '[' → ThisMonth
        m.handle_key(key(KeyCode::Char('[')));
        m.handle_key(key(KeyCode::Char('[')));
        m.handle_key(key(KeyCode::Char('[')));
        assert_eq!(m.date_range(), DateRangePreset::ThisMonth);
    }

    #[test]
    fn test_bracket_right_left_reverse() {
        let mut m = make_mounted_module();
        m.handle_key(key(KeyCode::Char(']')));
        assert_eq!(m.date_range(), DateRangePreset::LastMonth);
        m.handle_key(key(KeyCode::Char('[')));
        assert_eq!(m.date_range(), DateRangePreset::ThisMonth);
    }

    // === Multiple events ===

    #[test]
    fn test_usage_loaded_clears_error() {
        let mut m = make_module();
        m.error_message = Some("old error".into());
        m.handle_event(&AppEvent::UsageLoaded(vec![]));
        assert!(m.error_message().is_none());
    }

    // === on_mount (lazy init) ===

    #[test]
    fn test_first_key_triggers_mount() {
        let (tx, mut rx) = test_action_channel();
        let mut m = UsageModule::new(tx);
        assert!(!m.mounted);

        // Any key triggers mount
        m.handle_key(key(KeyCode::Char('j')));
        assert!(m.mounted);
        assert!(m.loading);

        // Should have sent FetchUsage, FetchHypervisors, FetchProjects
        let mut actions = Vec::new();
        while let Ok(a) = rx.try_recv() {
            actions.push(a);
        }
        assert_eq!(actions.len(), 3);
        assert!(matches!(actions[0], Action::FetchUsage { .. }));
        assert!(matches!(actions[1], Action::FetchHypervisors));
        assert!(matches!(actions[2], Action::FetchProjects));
    }

    #[test]
    fn test_second_key_does_not_remount() {
        let (tx, mut rx) = test_action_channel();
        let mut m = UsageModule::new(tx);

        m.handle_key(key(KeyCode::Char('j')));
        // Drain mount actions
        while rx.try_recv().is_ok() {}

        m.handle_key(key(KeyCode::Char('j')));
        // No new mount actions
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_loading_set_on_key_r() {
        let mut m = make_module();
        m.loading = false;
        m.handle_key(key(KeyCode::Char('r')));
        assert!(m.loading);
    }
}
