use crate::models::neutron::Network;
use crate::port::types::Subnet;
use crate::ui::detail_view::{DetailData, DetailField, DetailSection};
use crate::ui::form::FormField;
use crate::ui::resource_list::{ColumnDef, ColumnWidth, Row, RowStyleHint};

pub fn network_columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef {
            name: "Name".into(),
            width: ColumnWidth::Percent(25),
            alignment: ratatui::layout::Alignment::Left,
        },
        ColumnDef {
            name: "Status".into(),
            width: ColumnWidth::Fixed(10),
            alignment: ratatui::layout::Alignment::Left,
        },
        ColumnDef {
            name: "Admin".into(),
            width: ColumnWidth::Fixed(8),
            alignment: ratatui::layout::Alignment::Center,
        },
        ColumnDef {
            name: "External".into(),
            width: ColumnWidth::Fixed(9),
            alignment: ratatui::layout::Alignment::Center,
        },
        ColumnDef {
            name: "Shared".into(),
            width: ColumnWidth::Fixed(8),
            alignment: ratatui::layout::Alignment::Center,
        },
        ColumnDef {
            name: "MTU".into(),
            width: ColumnWidth::Fixed(6),
            alignment: ratatui::layout::Alignment::Right,
        },
    ]
}

pub fn network_to_row(network: &Network) -> Row {
    let (icon, style) = network_status_display(&network.status);
    let admin_label = if network.admin_state_up { "UP" } else { "DOWN" };
    let external_icon = if network.external { "✓" } else { "✗" };
    let shared_icon = if network.shared { "✓" } else { "✗" };
    let mtu_str = network.mtu.map(|m| m.to_string()).unwrap_or("-".into());

    Row {
        id: network.id.clone(),
        cells: vec![
            network.name.clone(),
            format!("{icon} {}", network.status),
            admin_label.to_string(),
            external_icon.to_string(),
            shared_icon.to_string(),
            mtu_str,
        ],
        style_hint: Some(style),
    }
}

pub fn network_status_display(status: &str) -> (&'static str, RowStyleHint) {
    match status.to_uppercase().as_str() {
        "ACTIVE" => ("●", RowStyleHint::Active),
        "ERROR" => ("✗", RowStyleHint::Error),
        "BUILD" => ("◐", RowStyleHint::Warning),
        "DOWN" => ("○", RowStyleHint::Disabled),
        _ => ("?", RowStyleHint::Normal),
    }
}

pub fn network_detail_data(network: &Network, subnets: &[Subnet]) -> DetailData {
    let mut sections = vec![];

    // Basic info
    let mut basic_fields = vec![
        DetailField::KeyValue {
            key: "ID".into(),
            value: network.id.clone(),
            style: None,
        },
        DetailField::KeyValue {
            key: "Name".into(),
            value: network.name.clone(),
            style: None,
        },
        DetailField::KeyValue {
            key: "Status".into(),
            value: network.status.clone(),
            style: Some(network_status_display(&network.status).1),
        },
    ];
    if let Some(ref desc) = network.description {
        basic_fields.push(DetailField::KeyValue {
            key: "Description".into(),
            value: desc.clone(),
            style: None,
        });
    }
    sections.push(DetailSection {
        name: "Basic Info".into(),
        fields: basic_fields,
    });

    // Settings
    let admin_label = if network.admin_state_up { "UP" } else { "DOWN" };
    let mut settings_fields = vec![
        DetailField::KeyValue {
            key: "Shared".into(),
            value: network.shared.to_string(),
            style: None,
        },
        DetailField::KeyValue {
            key: "External".into(),
            value: network.external.to_string(),
            style: None,
        },
        DetailField::KeyValue {
            key: "Admin State".into(),
            value: admin_label.to_string(),
            style: None,
        },
    ];
    if let Some(mtu) = network.mtu {
        settings_fields.push(DetailField::KeyValue {
            key: "MTU".into(),
            value: mtu.to_string(),
            style: None,
        });
    }
    if let Some(pse) = network.port_security_enabled {
        settings_fields.push(DetailField::KeyValue {
            key: "Port Security".into(),
            value: pse.to_string(),
            style: None,
        });
    }
    sections.push(DetailSection {
        name: "Settings".into(),
        fields: settings_fields,
    });

    // Provider
    let mut provider_fields = Vec::new();
    if let Some(ref nt) = network.provider_network_type {
        provider_fields.push(DetailField::KeyValue {
            key: "Network Type".into(),
            value: nt.clone(),
            style: None,
        });
    }
    if let Some(ref pn) = network.provider_physical_network {
        provider_fields.push(DetailField::KeyValue {
            key: "Physical Network".into(),
            value: pn.clone(),
            style: None,
        });
    }
    if let Some(seg) = network.provider_segmentation_id {
        provider_fields.push(DetailField::KeyValue {
            key: "Segmentation ID".into(),
            value: seg.to_string(),
            style: None,
        });
    }
    if !provider_fields.is_empty() {
        sections.push(DetailSection {
            name: "Provider".into(),
            fields: provider_fields,
        });
    }

    // Subnets
    if !subnets.is_empty() {
        let columns = vec![
            "Name".into(),
            "CIDR".into(),
            "Gateway".into(),
            "IP Version".into(),
        ];
        let rows: Vec<Vec<String>> = subnets
            .iter()
            .map(|s| {
                vec![
                    s.name.clone(),
                    s.cidr.clone(),
                    s.gateway_ip.as_deref().unwrap_or("-").to_string(),
                    s.ip_version.to_string(),
                ]
            })
            .collect();
        sections.push(DetailSection {
            name: "Subnets".into(),
            fields: vec![DetailField::NestedTable {
                label: "Subnets".into(),
                columns,
                rows,
            }],
        });
    }

    DetailData {
        title: format!("Network: {}", network.name),
        sections,
    }
}

pub fn network_create_form() -> Vec<FormField> {
    vec![
        FormField::text("Name", true),
        FormField::checkbox("Admin State Up"),
        FormField::checkbox("Shared"),
        FormField::checkbox("External"),
        FormField::text("MTU", false),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_network() -> Network {
        Network {
            id: "net-1".into(),
            name: "private-net".into(),
            status: "ACTIVE".into(),
            description: Some("Test network".into()),
            admin_state_up: true,
            external: false,
            shared: false,
            mtu: Some(1500),
            port_security_enabled: Some(true),
            subnets: vec!["sub-1".into()],
            provider_network_type: Some("vxlan".into()),
            provider_physical_network: None,
            provider_segmentation_id: Some(100),
        }
    }

    fn make_subnet() -> Subnet {
        Subnet {
            id: "sub-1".into(),
            name: "private-subnet".into(),
            network_id: "net-1".into(),
            cidr: "10.0.0.0/24".into(),
            ip_version: 4,
            gateway_ip: Some("10.0.0.1".into()),
        }
    }

    #[test]
    fn test_network_columns_count() {
        assert_eq!(network_columns().len(), 6);
    }

    #[test]
    fn test_network_to_row() {
        let net = make_network();
        let row = network_to_row(&net);
        assert_eq!(row.id, "net-1");
        assert_eq!(row.cells[0], "private-net");
        assert!(row.cells[1].contains("ACTIVE"));
        assert_eq!(row.cells[2], "UP");
        assert_eq!(row.cells[3], "✗"); // not external
        assert_eq!(row.cells[4], "✗"); // not shared
        assert_eq!(row.cells[5], "1500");
        assert_eq!(row.style_hint, Some(RowStyleHint::Active));
    }

    #[test]
    fn test_network_status_display() {
        assert_eq!(network_status_display("ACTIVE"), ("●", RowStyleHint::Active));
        assert_eq!(network_status_display("ERROR"), ("✗", RowStyleHint::Error));
        assert_eq!(network_status_display("BUILD"), ("◐", RowStyleHint::Warning));
        assert_eq!(network_status_display("DOWN"), ("○", RowStyleHint::Disabled));
        assert_eq!(network_status_display("UNKNOWN"), ("?", RowStyleHint::Normal));
    }

    #[test]
    fn test_network_detail_data() {
        let net = make_network();
        let subnets = vec![make_subnet()];
        let data = network_detail_data(&net, &subnets);
        assert_eq!(data.title, "Network: private-net");
        assert!(data.sections.len() >= 3); // Basic, Settings, Provider, Subnets
    }

    #[test]
    fn test_network_detail_data_no_subnets() {
        let net = make_network();
        let data = network_detail_data(&net, &[]);
        // No Subnets section when empty
        assert!(data.sections.iter().all(|s| s.name != "Subnets"));
    }

    #[test]
    fn test_network_create_form() {
        let form = network_create_form();
        assert_eq!(form.len(), 5);
        assert!(form[0].required); // Name
        assert!(!form[4].required); // MTU
    }
}
