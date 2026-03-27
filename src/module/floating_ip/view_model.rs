use crate::models::neutron::FloatingIp;
use crate::ui::form::FieldDef;
use crate::ui::resource_list::{ColumnDef, ColumnWidth, Row, RowStyleHint};

pub fn fip_columns(show_tenant: bool) -> Vec<ColumnDef> {
    let mut cols = vec![
        ColumnDef {
            name: "IP Address".into(),
            width: ColumnWidth::Percent(20),
            alignment: ratatui::layout::Alignment::Left,
        },
    ];
    if show_tenant {
        cols.push(ColumnDef {
            name: "Project".into(),
            width: ColumnWidth::Percent(12),
            alignment: ratatui::layout::Alignment::Left,
        });
    }
    cols.extend([
        ColumnDef {
            name: "Status".into(),
            width: ColumnWidth::Fixed(10),
            alignment: ratatui::layout::Alignment::Left,
        },
        ColumnDef {
            name: "Fixed IP".into(),
            width: ColumnWidth::Percent(20),
            alignment: ratatui::layout::Alignment::Left,
        },
        ColumnDef {
            name: "Network ID".into(),
            width: ColumnWidth::Percent(30),
            alignment: ratatui::layout::Alignment::Left,
        },
    ]);
    cols
}

pub fn fip_to_row(fip: &FloatingIp, show_tenant: bool) -> Row {
    let (icon, style) = fip_status_display(&fip.status);
    let fixed_ip = fip.fixed_ip_address.as_deref().unwrap_or("-");
    let mut cells = vec![
        fip.floating_ip_address.clone(),
    ];
    if show_tenant {
        cells.push(fip.tenant_id.as_deref().unwrap_or("-").to_string());
    }
    cells.extend([
        format!("{icon} {}", fip.status),
        fixed_ip.to_string(),
        fip.floating_network_id.clone(),
    ]);
    Row {
        id: fip.id.clone(),
        cells,
        style_hint: Some(style),
    }
}

/// Create floating IP form fields using FieldDef API.
/// The External Network dropdown options can be populated later via set_field_options.
pub fn fip_create_defs() -> Vec<FieldDef> {
    vec![
        FieldDef::dropdown("External Network", vec![], true),
    ]
}

pub fn fip_status_display(status: &str) -> (&'static str, RowStyleHint) {
    match status.to_uppercase().as_str() {
        "ACTIVE" => ("●", RowStyleHint::Active),
        "ERROR" => ("✗", RowStyleHint::Error),
        "DOWN" => ("○", RowStyleHint::Disabled),
        _ => ("?", RowStyleHint::Normal),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_fip() -> FloatingIp {
        FloatingIp {
            id: "fip-1".into(),
            floating_ip_address: "203.0.113.10".into(),
            status: "ACTIVE".into(),
            port_id: Some("port-1".into()),
            floating_network_id: "ext-net-1".into(),
            fixed_ip_address: Some("10.0.0.5".into()),
            router_id: Some("router-1".into()),
            tenant_id: None,
        }
    }

    #[test]
    fn test_fip_columns_count() {
        assert_eq!(fip_columns(false).len(), 4);
        assert_eq!(fip_columns(true).len(), 5);
        assert_eq!(fip_columns(true)[1].name, "Project");
    }

    #[test]
    fn test_fip_to_row() {
        let fip = make_fip();
        let row = fip_to_row(&fip, false);
        assert_eq!(row.id, "fip-1");
        assert_eq!(row.cells[0], "203.0.113.10");
        assert!(row.cells[1].contains("ACTIVE"));
        assert_eq!(row.cells[2], "10.0.0.5");
        assert_eq!(row.cells[3], "ext-net-1");
        assert_eq!(row.style_hint, Some(RowStyleHint::Active));
    }

    #[test]
    fn test_fip_to_row_no_fixed_ip() {
        let mut fip = make_fip();
        fip.fixed_ip_address = None;
        let row = fip_to_row(&fip, false);
        assert_eq!(row.cells[2], "-");
    }

    #[test]
    fn test_fip_create_defs() {
        use crate::ui::form::Validation;
        let defs = fip_create_defs();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name(), "External Network");
        assert!(defs[0].validations().contains(&Validation::Required));
        assert!(matches!(defs[0], FieldDef::Dropdown { .. }));
    }

    #[test]
    fn test_fip_status_display() {
        assert_eq!(fip_status_display("ACTIVE"), ("●", RowStyleHint::Active));
        assert_eq!(fip_status_display("ERROR"), ("✗", RowStyleHint::Error));
        assert_eq!(fip_status_display("DOWN"), ("○", RowStyleHint::Disabled));
        assert_eq!(fip_status_display("BUILD"), ("?", RowStyleHint::Normal));
        assert_eq!(fip_status_display("UNKNOWN"), ("?", RowStyleHint::Normal));
    }
}
