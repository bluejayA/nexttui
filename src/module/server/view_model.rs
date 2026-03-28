use std::collections::HashMap;

use crate::models::nova::{Address, Server};
use crate::ui::detail_view::{DetailData, DetailField, DetailSection};
use crate::ui::form::FieldDef;
use crate::ui::resource_list::{ColumnDef, ColumnWidth, Row, RowStyleHint};

pub fn server_columns(show_tenant: bool) -> Vec<ColumnDef> {
    let mut cols = vec![
        ColumnDef {
            name: "".into(),
            width: ColumnWidth::Fixed(3),
            alignment: ratatui::layout::Alignment::Center,
        },
        ColumnDef {
            name: "Name".into(),
            width: ColumnWidth::Percent(25),
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
            width: ColumnWidth::Fixed(12),
            alignment: ratatui::layout::Alignment::Left,
        },
        ColumnDef {
            name: "IP".into(),
            width: ColumnWidth::Percent(20),
            alignment: ratatui::layout::Alignment::Left,
        },
        ColumnDef {
            name: "Flavor".into(),
            width: ColumnWidth::Percent(15),
            alignment: ratatui::layout::Alignment::Left,
        },
        ColumnDef {
            name: "Image".into(),
            width: ColumnWidth::Percent(15),
            alignment: ratatui::layout::Alignment::Left,
        },
    ]);
    cols
}

pub fn server_to_row(server: &Server, show_tenant: bool) -> Row {
    let (icon, style) = status_display(&server.status);
    let flavor_name = server
        .flavor
        .original_name
        .as_deref()
        .unwrap_or(&server.flavor.id);
    let image_name = server
        .image
        .as_ref()
        .map(|i| i.id.as_str())
        .unwrap_or("-");
    let ips = format_ips(&server.addresses);

    let mut cells = vec![
        icon.to_string(),
        server.name.clone(),
    ];
    if show_tenant {
        cells.push(server.tenant_id.as_deref().unwrap_or("-").to_string());
    }
    cells.extend([
        server.status.clone(),
        ips,
        flavor_name.to_string(),
        image_name.to_string(),
    ]);

    Row {
        id: server.id.clone(),
        cells,
        style_hint: Some(style),
    }
}

pub fn server_detail_data(server: &Server) -> DetailData {
    let mut sections = vec![];

    // Basic info
    let mut basic_fields = vec![
        DetailField::KeyValue {
            key: "ID".into(),
            value: server.id.clone(),
            style: None,
        },
        DetailField::KeyValue {
            key: "Name".into(),
            value: server.name.clone(),
            style: None,
        },
        DetailField::KeyValue {
            key: "Status".into(),
            value: server.status.clone(),
            style: Some(status_display(&server.status).1),
        },
    ];
    if let Some(ref az) = server.availability_zone {
        basic_fields.push(DetailField::KeyValue {
            key: "AZ".into(),
            value: az.clone(),
            style: None,
        });
    }
    if let Some(ref key) = server.key_name {
        basic_fields.push(DetailField::KeyValue {
            key: "Keypair".into(),
            value: key.clone(),
            style: None,
        });
    }
    basic_fields.push(DetailField::KeyValue {
        key: "Created".into(),
        value: server.created.clone(),
        style: None,
    });
    if let Some(ref host) = server.host {
        basic_fields.push(DetailField::KeyValue {
            key: "Host".into(),
            value: host.clone(),
            style: None,
        });
    }
    sections.push(DetailSection {
        name: "Basic Info".into(),
        fields: basic_fields,
    });

    // Hardware
    let flavor_name = server
        .flavor
        .original_name
        .as_deref()
        .unwrap_or(&server.flavor.id);
    let hw_fields = vec![
        DetailField::KeyValue {
            key: "Flavor".into(),
            value: flavor_name.to_string(),
            style: None,
        },
        DetailField::KeyValue {
            key: "vCPU".into(),
            value: server
                .flavor
                .vcpus
                .map(|v| v.to_string())
                .unwrap_or("-".into()),
            style: None,
        },
        DetailField::KeyValue {
            key: "RAM".into(),
            value: server
                .flavor
                .ram
                .map(|r| format!("{} MB", r))
                .unwrap_or("-".into()),
            style: None,
        },
        DetailField::KeyValue {
            key: "Disk".into(),
            value: server
                .flavor
                .disk
                .map(|d| format!("{} GB", d))
                .unwrap_or("-".into()),
            style: None,
        },
    ];
    sections.push(DetailSection {
        name: "Hardware".into(),
        fields: hw_fields,
    });

    // Network (nested table)
    let net_columns = vec!["Network".into(), "IP".into(), "Type".into(), "MAC".into()];
    let mut net_rows = Vec::new();
    for (net_name, addrs) in &server.addresses {
        for addr in addrs {
            net_rows.push(vec![
                net_name.clone(),
                addr.addr.clone(),
                addr.ip_type.clone().unwrap_or("-".into()),
                addr.mac_addr.clone().unwrap_or("-".into()),
            ]);
        }
    }
    if !net_rows.is_empty() {
        sections.push(DetailSection {
            name: "Networks".into(),
            fields: vec![DetailField::NestedTable {
                label: "Addresses".into(),
                columns: net_columns,
                rows: net_rows,
            }],
        });
    }

    DetailData {
        title: format!("Server: {}", server.name),
        sections,
    }
}

/// Create server form fields using new FieldDef API.
/// Options for Image/Flavor/Network/SecurityGroup can be populated later via set_field_options.
pub fn server_create_defs() -> Vec<FieldDef> {
    vec![
        FieldDef::text("Name", true),
        FieldDef::dropdown("Image", vec![], true),
        FieldDef::dropdown("Flavor", vec![], true),
        FieldDef::dropdown("Network", vec![], true),
        FieldDef::dropdown("Security Group", vec![], false),
        FieldDef::text("Key Pair", false),
        FieldDef::text("Availability Zone", false),
    ]
}

/// Returns (icon, style_hint) for a server status string.
pub fn status_display(status: &str) -> (&'static str, RowStyleHint) {
    match status.to_uppercase().as_str() {
        "ACTIVE" => ("●", RowStyleHint::Active),
        "ERROR" | "DELETED" => ("✗", RowStyleHint::Error),
        "BUILD" | "RESIZE" | "REBOOT" | "REBUILD" | "MIGRATING" | "VERIFY_RESIZE" | "REVERT_RESIZE" => {
            ("◐", RowStyleHint::Warning)
        }
        "SHUTOFF" | "SUSPENDED" | "PAUSED" | "SHELVED" | "SHELVED_OFFLOADED" => {
            ("○", RowStyleHint::Disabled)
        }
        _ => ("?", RowStyleHint::Normal),
    }
}

pub fn format_ips(addresses: &HashMap<String, Vec<Address>>) -> String {
    let mut fixed = Vec::new();
    let mut floating = Vec::new();
    // Sort network keys for deterministic ordering across renders
    let mut keys: Vec<&String> = addresses.keys().collect();
    keys.sort();
    for key in keys {
        if let Some(addrs) = addresses.get(key) {
            for addr in addrs {
                match addr.ip_type.as_deref() {
                    Some("floating") => floating.push(addr.addr.as_str()),
                    _ => fixed.push(addr.addr.as_str()),
                }
            }
        }
    }
    let mut all: Vec<&str> = fixed;
    all.extend(floating);
    all.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::nova::{FlavorRef, ImageRef};
    use crate::ui::form::Validation;

    fn make_server(status: &str) -> Server {
        Server {
            id: "srv-1".into(),
            name: "web-01".into(),
            status: status.into(),
            addresses: {
                let mut m = HashMap::new();
                m.insert(
                    "private".into(),
                    vec![
                        Address {
                            addr: "10.0.0.5".into(),
                            version: 4,
                            mac_addr: Some("fa:16:3e:aa:bb:cc".into()),
                            ip_type: Some("fixed".into()),
                        },
                        Address {
                            addr: "192.168.1.100".into(),
                            version: 4,
                            mac_addr: None,
                            ip_type: Some("floating".into()),
                        },
                    ],
                );
                m
            },
            flavor: FlavorRef {
                id: "flv-1".into(),
                original_name: Some("m1.small".into()),
                vcpus: Some(2),
                ram: Some(4096),
                disk: Some(40),
            },
            image: Some(ImageRef { id: "img-1".into() }),
            key_name: Some("mykey".into()),
            availability_zone: Some("nova".into()),
            created: "2026-01-01T00:00:00Z".into(),
            updated: None,
            tenant_id: Some("proj-1".into()),
            host_id: None,
            host: Some("compute-01".into()),
        }
    }

    #[test]
    fn test_server_columns_count() {
        assert_eq!(server_columns(false).len(), 6);
        assert_eq!(server_columns(true).len(), 7);
        assert_eq!(server_columns(true)[2].name, "Project");
    }

    #[test]
    fn test_server_to_row_active() {
        let server = make_server("ACTIVE");
        let row = server_to_row(&server, false);
        assert_eq!(row.id, "srv-1");
        assert_eq!(row.cells[1], "web-01");
        assert_eq!(row.cells[2], "ACTIVE");
        assert_eq!(row.style_hint, Some(RowStyleHint::Active));
    }

    #[test]
    fn test_server_to_row_with_tenant() {
        let server = make_server("ACTIVE");
        let row = server_to_row(&server, true);
        assert_eq!(row.cells[1], "web-01");
        assert_eq!(row.cells[2], "proj-1");
        assert_eq!(row.cells[3], "ACTIVE");
    }

    #[test]
    fn test_server_to_row_error() {
        let server = make_server("ERROR");
        let row = server_to_row(&server, false);
        assert_eq!(row.style_hint, Some(RowStyleHint::Error));
    }

    #[test]
    fn test_format_ips_fixed_and_floating() {
        let server = make_server("ACTIVE");
        let ips = format_ips(&server.addresses);
        // fixed first, then floating
        assert!(ips.contains("10.0.0.5"));
        assert!(ips.contains("192.168.1.100"));
        let fixed_pos = ips.find("10.0.0.5").unwrap();
        let float_pos = ips.find("192.168.1.100").unwrap();
        assert!(fixed_pos < float_pos);
    }

    #[test]
    fn test_format_ips_empty() {
        let ips = format_ips(&HashMap::new());
        assert!(ips.is_empty());
    }

    #[test]
    fn test_status_display_mapping() {
        assert_eq!(status_display("ACTIVE"), ("●", RowStyleHint::Active));
        assert_eq!(status_display("ERROR"), ("✗", RowStyleHint::Error));
        assert_eq!(status_display("DELETED"), ("✗", RowStyleHint::Error));
        assert_eq!(status_display("BUILD"), ("◐", RowStyleHint::Warning));
        assert_eq!(status_display("SHUTOFF"), ("○", RowStyleHint::Disabled));
        assert_eq!(status_display("UNKNOWN"), ("?", RowStyleHint::Normal));
        assert_eq!(
            status_display("REVERT_RESIZE"),
            ("◐", RowStyleHint::Warning)
        );
        assert_eq!(status_display("MIGRATING"), ("◐", RowStyleHint::Warning));
        assert_eq!(
            status_display("VERIFY_RESIZE"),
            ("◐", RowStyleHint::Warning)
        );
    }

    #[test]
    fn test_server_detail_data_sections() {
        let server = make_server("ACTIVE");
        let data = server_detail_data(&server);
        assert_eq!(data.title, "Server: web-01");
        assert!(data.sections.len() >= 3); // Basic, Hardware, Networks
    }

    #[test]
    fn test_server_create_defs() {
        let defs = server_create_defs();
        assert_eq!(defs.len(), 7);
        assert_eq!(defs[0].name(), "Name");
        assert!(defs[0].validations().contains(&Validation::Required));
        assert_eq!(defs[1].name(), "Image");
        assert!(matches!(defs[1], FieldDef::Dropdown { .. }));
        assert_eq!(defs[2].name(), "Flavor");
        assert_eq!(defs[3].name(), "Network");
        assert_eq!(defs[4].name(), "Security Group");
        assert!(!defs[4].validations().contains(&Validation::Required));
        assert_eq!(defs[5].name(), "Key Pair");
        assert_eq!(defs[6].name(), "Availability Zone");
    }
}
