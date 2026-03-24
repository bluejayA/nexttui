use std::collections::HashMap;

use crate::models::nova::{Address, Server, Flavor};
use crate::ui::detail_view::{DetailData, DetailField, DetailSection};
use crate::ui::form::{FieldDef, FormField};
use crate::ui::resource_list::{ColumnDef, ColumnWidth, Row, RowStyleHint};

pub fn server_columns() -> Vec<ColumnDef> {
    vec![
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
    ]
}

pub fn server_to_row(server: &Server) -> Row {
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

    Row {
        id: server.id.clone(),
        cells: vec![
            icon.to_string(),
            server.name.clone(),
            server.status.clone(),
            ips,
            flavor_name.to_string(),
            image_name.to_string(),
        ],
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

pub fn server_create_form(
    flavors: &[Flavor],
    images: &[String],
    networks: &[String],
    security_groups: &[String],
) -> Vec<FormField> {
    vec![
        FormField::text("Name", true),
        FormField::dropdown(
            "Image",
            images.to_vec(),
            true,
        ),
        FormField::dropdown(
            "Flavor",
            flavors.iter().map(|f| format!("{} ({}vCPU/{}MB/{}GB)", f.name, f.vcpus, f.ram, f.disk)).collect(),
            true,
        ),
        FormField::dropdown(
            "Network",
            networks.to_vec(),
            true,
        ),
        FormField::dropdown(
            "Security Group",
            security_groups.to_vec(),
            false,
        ),
        FormField::text("Key Pair", false),
        FormField::text("Availability Zone", false),
    ]
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
        "BUILD" | "RESIZE" | "REBOOT" | "REBUILD" | "MIGRATING" | "VERIFY_RESIZE" => {
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
    use crate::ui::form::FormFieldType;

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
        assert_eq!(server_columns().len(), 6);
    }

    #[test]
    fn test_server_to_row_active() {
        let server = make_server("ACTIVE");
        let row = server_to_row(&server);
        assert_eq!(row.id, "srv-1");
        assert_eq!(row.cells[1], "web-01");
        assert_eq!(row.cells[2], "ACTIVE");
        assert_eq!(row.style_hint, Some(RowStyleHint::Active));
    }

    #[test]
    fn test_server_to_row_error() {
        let server = make_server("ERROR");
        let row = server_to_row(&server);
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
    }

    #[test]
    fn test_server_detail_data_sections() {
        let server = make_server("ACTIVE");
        let data = server_detail_data(&server);
        assert_eq!(data.title, "Server: web-01");
        assert!(data.sections.len() >= 3); // Basic, Hardware, Networks
    }

    #[test]
    fn test_server_create_form_fields() {
        let flavors = vec![Flavor {
            id: "flv-1".into(),
            name: "m1.small".into(),
            vcpus: 1,
            ram: 2048,
            disk: 20,
            is_public: true,
        }];
        let form = server_create_form(&flavors, &["img-1".into()], &["net-1".into()], &["default".into()]);
        assert_eq!(form.len(), 7);
        assert!(form[0].required); // Name
        assert!(matches!(form[1].field_type, FormFieldType::Dropdown(_))); // Image
    }
}
