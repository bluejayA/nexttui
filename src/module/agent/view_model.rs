use crate::models::neutron::NetworkAgent;
use crate::ui::resource_list::{ColumnDef, ColumnWidth, Row, RowStyleHint};

pub fn agent_columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef {
            name: "Type".into(),
            width: ColumnWidth::Percent(25),
            alignment: ratatui::layout::Alignment::Left,
        },
        ColumnDef {
            name: "Host".into(),
            width: ColumnWidth::Percent(20),
            alignment: ratatui::layout::Alignment::Left,
        },
        ColumnDef {
            name: "Admin".into(),
            width: ColumnWidth::Fixed(10),
            alignment: ratatui::layout::Alignment::Center,
        },
        ColumnDef {
            name: "Alive".into(),
            width: ColumnWidth::Fixed(8),
            alignment: ratatui::layout::Alignment::Center,
        },
        ColumnDef {
            name: "Binary".into(),
            width: ColumnWidth::Percent(20),
            alignment: ratatui::layout::Alignment::Left,
        },
    ]
}

pub fn agent_to_row(agent: &NetworkAgent) -> Row {
    let admin_label = if agent.admin_state_up {
        "Enabled"
    } else {
        "Disabled"
    };
    let alive_icon = if agent.alive { "✓" } else { "✗" };
    let style = if agent.alive && agent.admin_state_up {
        RowStyleHint::Active
    } else if !agent.alive {
        RowStyleHint::Error
    } else {
        RowStyleHint::Disabled
    };
    Row {
        id: agent.id.clone(),
        cells: vec![
            agent.agent_type.clone(),
            agent.host.clone(),
            admin_label.to_string(),
            alive_icon.to_string(),
            agent.binary.clone(),
        ],
        style_hint: Some(style),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_columns() {
        assert_eq!(agent_columns().len(), 5);
    }
    #[test]
    fn test_to_row_alive() {
        let a = NetworkAgent {
            id: "a1".into(),
            agent_type: "OVS".into(),
            host: "n1".into(),
            admin_state_up: true,
            alive: true,
            binary: "neutron-ovs".into(),
        };
        let row = agent_to_row(&a);
        assert_eq!(row.cells[2], "Enabled");
        assert_eq!(row.cells[3], "✓");
        assert_eq!(row.style_hint, Some(RowStyleHint::Active));
    }
    #[test]
    fn test_to_row_dead() {
        let a = NetworkAgent {
            id: "a1".into(),
            agent_type: "OVS".into(),
            host: "n1".into(),
            admin_state_up: true,
            alive: false,
            binary: "neutron-ovs".into(),
        };
        assert_eq!(agent_to_row(&a).style_hint, Some(RowStyleHint::Error));
    }
}
