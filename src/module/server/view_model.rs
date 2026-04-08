use std::collections::HashMap;

use crate::models::nova::{Address, Flavor, Server, ServerMigration};
use crate::ui::detail_view::{DetailData, DetailField, DetailSection};
use crate::ui::form::FieldDef;
use crate::ui::resource_list::{ColumnDef, ColumnWidth, Row, RowStyleHint};

pub fn server_columns(show_tenant: bool) -> Vec<ColumnDef> {
    server_columns_full(show_tenant, false)
}

pub fn server_columns_full(show_tenant: bool, show_host: bool) -> Vec<ColumnDef> {
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
    if show_host {
        cols.push(ColumnDef {
            name: "Host".into(),
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
            width: ColumnWidth::Percent(13),
            alignment: ratatui::layout::Alignment::Left,
        },
        ColumnDef {
            name: "Vol".into(),
            width: ColumnWidth::Fixed(4),
            alignment: ratatui::layout::Alignment::Center,
        },
    ]);
    cols
}

pub fn server_to_row(server: &Server, show_tenant: bool) -> Row {
    server_to_row_full(server, show_tenant, false)
}

pub fn server_to_row_full(server: &Server, show_tenant: bool, show_host: bool) -> Row {
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
    if show_host {
        cells.push(server.host.as_deref().unwrap_or("-").to_string());
    }
    let vol_count = server.volumes_attached.len();
    let vol_display = if vol_count > 0 {
        vol_count.to_string()
    } else {
        "-".into()
    };
    cells.extend([
        server.status.clone(),
        ips,
        flavor_name.to_string(),
        image_name.to_string(),
        vol_display,
    ]);

    Row {
        id: server.id.clone(),
        cells,
        style_hint: Some(style),
    }
}

pub fn server_detail_data(server: &Server) -> DetailData {
    server_detail_data_full(server, None, None, false, &[], &[])
}

pub fn server_detail_data_full(
    server: &Server,
    migration_progress: Option<&ServerMigration>,
    flavor: Option<&Flavor>,
    is_resize_pending: bool,
    cached_volumes: &[crate::models::cinder::Volume],
    cached_floating_ips: &[crate::models::neutron::FloatingIp],
) -> DetailData {
    let mut sections = vec![];

    // VERIFY_RESIZE banner — distinguish resize vs migration
    if server.status == "VERIFY_RESIZE" {
        let (banner_title, banner_text) = if is_resize_pending {
            ("⚠ Resize Pending", "Confirm(Y) or Revert(N) resize")
        } else {
            ("⚠ Migration Pending", "Confirm(Y) or Revert(N) migration")
        };
        sections.push(DetailSection {
            name: banner_title.into(),
            fields: vec![DetailField::KeyValue {
                key: "Action Required".into(),
                value: banner_text.into(),
                style: Some(RowStyleHint::Warning),
            }],
        });
    }

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
        let (host_value, host_style) = if let Some(mig) = migration_progress {
            (
                format!("{} → {}", mig.source_compute, mig.dest_compute),
                Some(RowStyleHint::Warning),
            )
        } else {
            (host.clone(), None)
        };
        basic_fields.push(DetailField::KeyValue {
            key: "Host".into(),
            value: host_value,
            style: host_style,
        });
    }
    sections.push(DetailSection {
        name: "Basic Info".into(),
        fields: basic_fields,
    });

    // Hardware — prefer cached Flavor data over server.flavor (which may lack details in older API versions)
    let (flavor_name, vcpus, ram, disk) = if let Some(f) = flavor {
        (
            f.name.clone(),
            f.vcpus.to_string(),
            format!("{} MB", f.ram),
            format!("{} GB", f.disk),
        )
    } else {
        (
            server.flavor.original_name.as_deref().unwrap_or(&server.flavor.id).to_string(),
            server.flavor.vcpus.map(|v| v.to_string()).unwrap_or("-".into()),
            server.flavor.ram.map(|r| format!("{} MB", r)).unwrap_or("-".into()),
            server.flavor.disk.map(|d| format!("{} GB", d)).unwrap_or("-".into()),
        )
    };
    let hw_fields = vec![
        DetailField::KeyValue {
            key: "Flavor".into(),
            value: flavor_name,
            style: None,
        },
        DetailField::KeyValue {
            key: "vCPU".into(),
            value: vcpus,
            style: None,
        },
        DetailField::KeyValue {
            key: "RAM".into(),
            value: ram,
            style: None,
        },
        DetailField::KeyValue {
            key: "Disk".into(),
            value: disk,
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

    // Floating IPs — match by floating IP address in server.addresses
    {
        let mut fip_fields = Vec::new();
        // Collect floating IPs from addresses
        let floating_addrs: Vec<&str> = server.addresses.values()
            .flat_map(|addrs| addrs.iter())
            .filter(|a| a.ip_type.as_deref() == Some("floating"))
            .map(|a| a.addr.as_str())
            .collect();

        // Match against cached FIPs for full info
        for addr in &floating_addrs {
            if let Some(fip) = cached_floating_ips.iter().find(|f| f.floating_ip_address == *addr) {
                fip_fields.push(DetailField::KeyValue {
                    key: "FIP".into(),
                    value: format!("{} → {}", fip.floating_ip_address,
                        fip.fixed_ip_address.as_deref().unwrap_or("-")),
                    style: Some(RowStyleHint::Active),
                });
                fip_fields.push(DetailField::KeyValue {
                    key: "  ID".into(),
                    value: fip.id.clone(),
                    style: None,
                });
                fip_fields.push(DetailField::KeyValue {
                    key: "  Status".into(),
                    value: fip.status.clone(),
                    style: None,
                });
            } else {
                // FIP found in addresses but not in cache — show address only
                fip_fields.push(DetailField::KeyValue {
                    key: "FIP".into(),
                    value: addr.to_string(),
                    style: Some(RowStyleHint::Active),
                });
            }
        }
        if !fip_fields.is_empty() {
            sections.push(DetailSection {
                name: "Floating IPs".into(),
                fields: fip_fields,
            });
        }
    }

    // Security Groups
    if !server.security_groups.is_empty() {
        let sg_names: Vec<String> = server.security_groups.iter().map(|sg| sg.name.clone()).collect();
        sections.push(DetailSection {
            name: "Security Groups".into(),
            fields: sg_names
                .iter()
                .map(|name| DetailField::KeyValue {
                    key: "SG".into(),
                    value: name.clone(),
                    style: None,
                })
                .collect(),
        });
    }

    // Attached Volumes
    if !server.volumes_attached.is_empty() {
        let vol_columns = vec!["Name".into(), "Size".into(), "Status".into(), "Device".into()];
        let mut vol_rows = Vec::new();
        for att_vol in &server.volumes_attached {
            // Try to resolve volume details from cached_volumes
            let cached = cached_volumes.iter().find(|v| v.id == att_vol.id);
            let name = cached
                .and_then(|v| v.name.as_deref())
                .unwrap_or(&att_vol.id[..8]);
            let size = cached
                .map(|v| format!("{} GB", v.size))
                .unwrap_or("-".into());
            let status = cached
                .map(|v| v.status.clone())
                .unwrap_or("-".into());
            let device = cached
                .and_then(|v| {
                    v.attachments.iter()
                        .find(|a| a.server_id == server.id)
                        .map(|a| a.device.clone())
                })
                .unwrap_or("-".into());
            vol_rows.push(vec![name.to_string(), size, status, device]);
        }
        sections.push(DetailSection {
            name: "Volumes".into(),
            fields: vec![DetailField::NestedTable {
                label: "Attached".into(),
                columns: vol_columns,
                rows: vol_rows,
            }],
        });
    }

    // Migration Progress
    if let Some(mig) = migration_progress {
        let mut mig_fields = vec![
            DetailField::KeyValue {
                key: "Status".into(),
                value: mig.status.clone(),
                style: None,
            },
            DetailField::KeyValue {
                key: "Source".into(),
                value: mig.source_compute.clone(),
                style: None,
            },
            DetailField::KeyValue {
                key: "Dest".into(),
                value: mig.dest_compute.clone(),
                style: None,
            },
        ];
        if let (Some(total), Some(processed)) =
            (mig.memory_total_bytes, mig.memory_processed_bytes)
        {
            let pct = if total > 0 { processed * 100 / total } else { 0 };
            mig_fields.push(DetailField::KeyValue {
                key: "Memory".into(),
                value: format!("{}% ({}/{})", pct, format_bytes(processed), format_bytes(total)),
                style: None,
            });
        }
        if let (Some(total), Some(processed)) =
            (mig.disk_total_bytes, mig.disk_processed_bytes)
        {
            let pct = if total > 0 { processed * 100 / total } else { 0 };
            mig_fields.push(DetailField::KeyValue {
                key: "Disk".into(),
                value: format!("{}% ({}/{})", pct, format_bytes(processed), format_bytes(total)),
                style: None,
            });
        }
        sections.push(DetailSection {
            name: "Migration Progress".into(),
            fields: mig_fields,
        });
    }

    DetailData {
        title: format!("Server: {}", server.name),
        sections,
    }
}

fn format_bytes(bytes: i64) -> String {
    const KB: i64 = 1024;
    const MB: i64 = KB * 1024;
    const GB: i64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
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
            volumes_attached: vec![],
            security_groups: vec![],
        }
    }

    #[test]
    fn test_server_columns_count() {
        assert_eq!(server_columns(false).len(), 7);
        assert_eq!(server_columns(true).len(), 8);
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

    #[test]
    fn test_server_columns_with_host() {
        let cols = server_columns_full(false, true);
        assert_eq!(cols.len(), 8); // base 7 + Host
        assert_eq!(cols[2].name, "Host");
    }

    #[test]
    fn test_server_columns_with_tenant_and_host() {
        let cols = server_columns_full(true, true);
        assert_eq!(cols.len(), 9); // base 7 + Project + Host
        assert_eq!(cols[2].name, "Project");
        assert_eq!(cols[3].name, "Host");
    }

    #[test]
    fn test_server_to_row_with_host() {
        let server = make_server("ACTIVE");
        let row = server_to_row_full(&server, false, true);
        // cells: icon, name, host, status, ip, flavor, image
        assert_eq!(row.cells[2], "compute-01");
        assert_eq!(row.cells[3], "ACTIVE");
    }

    #[test]
    fn test_detail_verify_resize_banner() {
        let server = make_server("VERIFY_RESIZE");
        let data = server_detail_data_full(&server, None, None, false, &[], &[]);
        assert_eq!(data.sections[0].name, "⚠ Migration Pending");
    }

    #[test]
    fn test_detail_no_banner_for_active() {
        let server = make_server("ACTIVE");
        let data = server_detail_data_full(&server, None, None, false, &[], &[]);
        assert_ne!(data.sections[0].name, "⚠ Migration Pending");
    }

    #[test]
    fn test_detail_migration_progress_section() {
        let server = make_server("MIGRATING");
        let mig = ServerMigration {
            id: 1,
            status: "running".into(),
            source_compute: "compute-01".into(),
            dest_compute: "compute-02".into(),
            memory_total_bytes: Some(1024 * 1024_i64),
            memory_processed_bytes: Some(512 * 1024_i64),
            memory_remaining_bytes: Some(512 * 1024_i64),
            disk_total_bytes: Some(10 * 1024 * 1024 * 1024_i64),
            disk_processed_bytes: Some(5 * 1024 * 1024 * 1024_i64),
            disk_remaining_bytes: Some(5 * 1024 * 1024 * 1024_i64),
            created_at: None,
            updated_at: None,
        };
        let data = server_detail_data_full(&server, Some(&mig), None, false, &[], &[]);
        let mig_section = data.sections.iter().find(|s| s.name == "Migration Progress");
        assert!(mig_section.is_some());
        let fields = &mig_section.unwrap().fields;
        // Status, Source, Dest, Memory, Disk = 5 fields
        assert_eq!(fields.len(), 5);
    }

    #[test]
    fn test_detail_no_migration_progress_without_data() {
        let server = make_server("ACTIVE");
        let data = server_detail_data_full(&server, None, None, false, &[], &[]);
        let mig_section = data.sections.iter().find(|s| s.name == "Migration Progress");
        assert!(mig_section.is_none());
    }

    #[test]
    fn test_format_bytes_units() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1048576), "1.0 MB");
        assert_eq!(format_bytes(1073741824), "1.0 GB");
    }

    #[test]
    fn test_server_to_row_host_none_shows_dash() {
        let mut server = make_server("ACTIVE");
        server.host = None;
        let row = server_to_row_full(&server, false, true);
        assert_eq!(row.cells[2], "-");
    }

    #[test]
    fn test_detail_migration_progress_memory_only() {
        let server = make_server("MIGRATING");
        let mig = ServerMigration {
            id: 1,
            status: "running".into(),
            source_compute: "c1".into(),
            dest_compute: "c2".into(),
            memory_total_bytes: Some(2048),
            memory_processed_bytes: Some(1024),
            memory_remaining_bytes: Some(1024),
            disk_total_bytes: None,
            disk_processed_bytes: None,
            disk_remaining_bytes: None,
            created_at: None,
            updated_at: None,
        };
        let data = server_detail_data_full(&server, Some(&mig), None, false, &[], &[]);
        let mig_section = data.sections.iter().find(|s| s.name == "Migration Progress").unwrap();
        // Status, Source, Dest, Memory = 4 fields (no Disk)
        assert_eq!(mig_section.fields.len(), 4);
    }

    #[test]
    fn test_detail_verify_resize_banner_migration() {
        let server = make_server("VERIFY_RESIZE");
        let data = server_detail_data_full(&server, None, None, false, &[], &[]);
        let banner = &data.sections[0];
        assert_eq!(banner.name, "⚠ Migration Pending");
        if let DetailField::KeyValue { value, style, .. } = &banner.fields[0] {
            assert!(value.contains("migration"));
            assert_eq!(*style, Some(RowStyleHint::Warning));
        } else {
            panic!("Expected KeyValue in banner");
        }
    }

    #[test]
    fn test_detail_verify_resize_banner_resize() {
        let server = make_server("VERIFY_RESIZE");
        let data = server_detail_data_full(&server, None, None, true, &[], &[]);
        let banner = &data.sections[0];
        assert_eq!(banner.name, "⚠ Resize Pending");
        if let DetailField::KeyValue { value, style, .. } = &banner.fields[0] {
            assert!(value.contains("resize"));
            assert!(!value.contains("migration"));
            assert_eq!(*style, Some(RowStyleHint::Warning));
        } else {
            panic!("Expected KeyValue in banner");
        }
    }

    #[test]
    fn test_detail_security_groups_section() {
        use crate::models::nova::ServerSecurityGroup;
        let mut server = make_server("ACTIVE");
        server.security_groups = vec![
            ServerSecurityGroup { name: "default".into() },
            ServerSecurityGroup { name: "web-sg".into() },
        ];
        let data = server_detail_data(&server);
        let sg_section = data.sections.iter().find(|s| s.name == "Security Groups");
        assert!(sg_section.is_some(), "Security Groups section should exist");
        let fields = &sg_section.unwrap().fields;
        assert_eq!(fields.len(), 2);
        if let DetailField::KeyValue { key, value, .. } = &fields[0] {
            assert_eq!(key, "SG");
            assert_eq!(value, "default");
        } else {
            panic!("Expected KeyValue for SG field");
        }
        if let DetailField::KeyValue { value, .. } = &fields[1] {
            assert_eq!(value, "web-sg");
        } else {
            panic!("Expected KeyValue for SG field");
        }
    }

    #[test]
    fn test_detail_no_security_groups_when_empty() {
        let server = make_server("ACTIVE");
        let data = server_detail_data(&server);
        let sg_section = data.sections.iter().find(|s| s.name == "Security Groups");
        assert!(sg_section.is_none(), "Security Groups section should not exist when empty");
    }
}
