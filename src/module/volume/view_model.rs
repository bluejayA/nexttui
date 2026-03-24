use crate::models::cinder::Volume;
use crate::ui::detail_view::{DetailData, DetailField, DetailSection};
use crate::ui::form::FieldDef;
use crate::ui::resource_list::{ColumnDef, ColumnWidth, Row, RowStyleHint};

pub fn volume_columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef {
            name: "".into(),
            width: ColumnWidth::Fixed(3),
            alignment: ratatui::layout::Alignment::Center,
        },
        ColumnDef {
            name: "Name".into(),
            width: ColumnWidth::Percent(20),
            alignment: ratatui::layout::Alignment::Left,
        },
        ColumnDef {
            name: "Status".into(),
            width: ColumnWidth::Fixed(12),
            alignment: ratatui::layout::Alignment::Left,
        },
        ColumnDef {
            name: "Size(GB)".into(),
            width: ColumnWidth::Fixed(9),
            alignment: ratatui::layout::Alignment::Right,
        },
        ColumnDef {
            name: "Type".into(),
            width: ColumnWidth::Fixed(12),
            alignment: ratatui::layout::Alignment::Left,
        },
        ColumnDef {
            name: "Bootable".into(),
            width: ColumnWidth::Fixed(9),
            alignment: ratatui::layout::Alignment::Center,
        },
        ColumnDef {
            name: "Attached To".into(),
            width: ColumnWidth::Percent(18),
            alignment: ratatui::layout::Alignment::Left,
        },
    ]
}

pub fn volume_to_row(volume: &Volume) -> Row {
    let (icon, style) = volume_status_display(&volume.status);
    let name = volume.name.as_deref().unwrap_or("-");
    let vol_type = volume.volume_type.as_deref().unwrap_or("-");
    let bootable_icon = if volume.bootable == "true" { "✓" } else { "✗" };
    let attached = if volume.attachments.is_empty() {
        "-".to_string()
    } else {
        volume
            .attachments
            .iter()
            .map(|a| {
                let short_id: String = a.server_id.chars().take(8).collect();
                format!("{short_id} ({})", a.device)
            })
            .collect::<Vec<_>>()
            .join(", ")
    };

    Row {
        id: volume.id.clone(),
        cells: vec![
            icon.to_string(),
            name.to_string(),
            volume.status.clone(),
            volume.size.to_string(),
            vol_type.to_string(),
            bootable_icon.to_string(),
            attached,
        ],
        style_hint: Some(style),
    }
}

pub fn volume_status_display(status: &str) -> (&'static str, RowStyleHint) {
    match status.to_lowercase().as_str() {
        "available" => ("●", RowStyleHint::Active),
        "in-use" => ("◆", RowStyleHint::Active),
        "error" | "error_deleting" | "error_extending" | "error_restoring" => {
            ("✗", RowStyleHint::Error)
        }
        "creating" | "attaching" | "detaching" | "extending" | "downloading" | "uploading"
        | "retyping" | "migrating" | "deleting" => ("◐", RowStyleHint::Warning),
        "maintenance" => ("○", RowStyleHint::Disabled),
        _ => ("?", RowStyleHint::Normal),
    }
}

pub fn volume_detail_data(volume: &Volume) -> DetailData {
    let mut sections = vec![];

    // Basic info
    let name = volume.name.as_deref().unwrap_or("-");
    let mut basic_fields = vec![
        DetailField::KeyValue {
            key: "ID".into(),
            value: volume.id.clone(),
            style: None,
        },
        DetailField::KeyValue {
            key: "Name".into(),
            value: name.to_string(),
            style: None,
        },
        DetailField::KeyValue {
            key: "Status".into(),
            value: volume.status.clone(),
            style: Some(volume_status_display(&volume.status).1),
        },
        DetailField::KeyValue {
            key: "Size".into(),
            value: format!("{} GB", volume.size),
            style: None,
        },
    ];
    if let Some(ref desc) = volume.description {
        basic_fields.push(DetailField::KeyValue {
            key: "Description".into(),
            value: desc.clone(),
            style: None,
        });
    }
    if let Some(ref vtype) = volume.volume_type {
        basic_fields.push(DetailField::KeyValue {
            key: "Type".into(),
            value: vtype.clone(),
            style: None,
        });
    }
    basic_fields.push(DetailField::KeyValue {
        key: "Encrypted".into(),
        value: volume.encrypted.to_string(),
        style: None,
    });
    basic_fields.push(DetailField::KeyValue {
        key: "Bootable".into(),
        value: volume.bootable.clone(),
        style: None,
    });
    if let Some(ref az) = volume.availability_zone {
        basic_fields.push(DetailField::KeyValue {
            key: "AZ".into(),
            value: az.clone(),
            style: None,
        });
    }
    if let Some(ref created) = volume.created_at {
        basic_fields.push(DetailField::KeyValue {
            key: "Created".into(),
            value: created.clone(),
            style: None,
        });
    }
    sections.push(DetailSection {
        name: "Basic Info".into(),
        fields: basic_fields,
    });

    // Attachments
    if !volume.attachments.is_empty() {
        let columns = vec!["Server ID".into(), "Device".into(), "Attachment ID".into()];
        let rows: Vec<Vec<String>> = volume
            .attachments
            .iter()
            .map(|a| vec![a.server_id.clone(), a.device.clone(), a.id.clone()])
            .collect();
        sections.push(DetailSection {
            name: "Attachments".into(),
            fields: vec![DetailField::NestedTable {
                label: "Attachments".into(),
                columns,
                rows,
            }],
        });
    }

    DetailData {
        title: format!("Volume: {}", name),
        sections,
    }
}

pub fn volume_create_defs() -> Vec<FieldDef> {
    vec![
        FieldDef::text("Name", true),
        FieldDef::text("Size (GB)", true),
        FieldDef::text("Type", false),
        FieldDef::text("Description", false),
        FieldDef::text("Availability Zone", false),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::cinder::VolumeAttachment;
    use crate::ui::form::Validation;

    fn make_volume() -> Volume {
        Volume {
            id: "vol-1".into(),
            name: Some("data-vol".into()),
            description: Some("Data volume".into()),
            status: "available".into(),
            size: 100,
            volume_type: Some("ssd".into()),
            encrypted: false,
            bootable: "false".into(),
            attachments: vec![],
            availability_zone: Some("az1".into()),
            created_at: Some("2026-01-01T00:00:00Z".into()),
        }
    }

    fn make_attached_volume() -> Volume {
        Volume {
            attachments: vec![VolumeAttachment {
                server_id: "srv-12345678-abcd".into(),
                device: "/dev/vdb".into(),
                id: "att-1".into(),
            }],
            status: "in-use".into(),
            ..make_volume()
        }
    }

    #[test]
    fn test_volume_columns_count() {
        assert_eq!(volume_columns().len(), 7);
    }

    #[test]
    fn test_volume_to_row_available() {
        let vol = make_volume();
        let row = volume_to_row(&vol);
        assert_eq!(row.id, "vol-1");
        assert_eq!(row.cells[1], "data-vol");
        assert_eq!(row.cells[2], "available");
        assert_eq!(row.cells[3], "100");
        assert_eq!(row.cells[4], "ssd");
        assert_eq!(row.cells[5], "✗"); // not bootable
        assert_eq!(row.cells[6], "-"); // no attachments
        assert_eq!(row.style_hint, Some(RowStyleHint::Active));
    }

    #[test]
    fn test_volume_to_row_attached() {
        let vol = make_attached_volume();
        let row = volume_to_row(&vol);
        assert!(row.cells[6].contains("srv-1234"));
        assert!(row.cells[6].contains("/dev/vdb"));
    }

    #[test]
    fn test_volume_status_display() {
        assert_eq!(volume_status_display("available"), ("●", RowStyleHint::Active));
        assert_eq!(volume_status_display("in-use"), ("◆", RowStyleHint::Active));
        assert_eq!(volume_status_display("error"), ("✗", RowStyleHint::Error));
        assert_eq!(volume_status_display("creating"), ("◐", RowStyleHint::Warning));
        assert_eq!(volume_status_display("maintenance"), ("○", RowStyleHint::Disabled));
        assert_eq!(volume_status_display("UNKNOWN"), ("?", RowStyleHint::Normal));
    }

    #[test]
    fn test_volume_detail_data() {
        let vol = make_volume();
        let data = volume_detail_data(&vol);
        assert_eq!(data.title, "Volume: data-vol");
        assert_eq!(data.sections.len(), 1); // Basic Info only (no attachments)
    }

    #[test]
    fn test_volume_detail_data_with_attachments() {
        let vol = make_attached_volume();
        let data = volume_detail_data(&vol);
        assert_eq!(data.sections.len(), 2); // Basic + Attachments
        assert!(data.sections.iter().any(|s| s.name == "Attachments"));
    }

    #[test]
    fn test_volume_create_defs() {
        let defs = volume_create_defs();
        assert_eq!(defs.len(), 5);
        assert_eq!(defs[0].name(), "Name");
        assert!(defs[0].validations().contains(&Validation::Required));
        assert_eq!(defs[1].name(), "Size (GB)");
        assert!(defs[1].validations().contains(&Validation::Required));
        assert_eq!(defs[2].name(), "Type");
        assert!(!defs[2].validations().contains(&Validation::Required));
        assert_eq!(defs[3].name(), "Description");
        assert_eq!(defs[4].name(), "Availability Zone");
    }
}
