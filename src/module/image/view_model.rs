use crate::models::glance::Image;
use crate::ui::detail_view::{DetailData, DetailField, DetailSection};
use crate::ui::form::FieldDef;
use crate::ui::resource_list::{ColumnDef, ColumnWidth, Row, RowStyleHint};

pub fn image_columns() -> Vec<ColumnDef> {
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
            name: "Format".into(),
            width: ColumnWidth::Fixed(10),
            alignment: ratatui::layout::Alignment::Left,
        },
        ColumnDef {
            name: "Size".into(),
            width: ColumnWidth::Fixed(10),
            alignment: ratatui::layout::Alignment::Right,
        },
        ColumnDef {
            name: "Visibility".into(),
            width: ColumnWidth::Fixed(12),
            alignment: ratatui::layout::Alignment::Left,
        },
    ]
}

pub fn image_to_row(image: &Image) -> Row {
    let (icon, style) = image_status_display(&image.status);
    let format = image.disk_format.as_deref().unwrap_or("-");
    let size_str = image
        .size
        .map(|s| format_bytes(s))
        .unwrap_or("-".to_string());
    Row {
        id: image.id.clone(),
        cells: vec![
            image.name.clone(),
            format!("{icon} {}", image.status),
            format.to_string(),
            size_str,
            image.visibility.clone(),
        ],
        style_hint: Some(style),
    }
}

pub fn image_status_display(status: &str) -> (&'static str, RowStyleHint) {
    match status.to_lowercase().as_str() {
        "active" => ("●", RowStyleHint::Active),
        "error" | "killed" | "deleted" => ("✗", RowStyleHint::Error),
        "saving" | "queued" | "importing" => ("◐", RowStyleHint::Warning),
        "deactivated" => ("○", RowStyleHint::Disabled),
        _ => ("?", RowStyleHint::Normal),
    }
}

fn format_bytes(bytes: u64) -> String {
    const GB: u64 = 1024 * 1024 * 1024;
    const MB: u64 = 1024 * 1024;
    const KB: u64 = 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

pub fn image_detail_data(image: &Image) -> DetailData {
    let mut sections = vec![];

    let mut basic_fields = vec![
        DetailField::KeyValue {
            key: "ID".into(),
            value: image.id.clone(),
            style: None,
        },
        DetailField::KeyValue {
            key: "Name".into(),
            value: image.name.clone(),
            style: None,
        },
        DetailField::KeyValue {
            key: "Status".into(),
            value: image.status.clone(),
            style: Some(image_status_display(&image.status).1),
        },
    ];
    if let Some(ref fmt) = image.disk_format {
        basic_fields.push(DetailField::KeyValue {
            key: "Disk Format".into(),
            value: fmt.clone(),
            style: None,
        });
    }
    if let Some(ref fmt) = image.container_format {
        basic_fields.push(DetailField::KeyValue {
            key: "Container Format".into(),
            value: fmt.clone(),
            style: None,
        });
    }
    if let Some(size) = image.size {
        basic_fields.push(DetailField::KeyValue {
            key: "Size".into(),
            value: format_bytes(size),
            style: None,
        });
    }
    if let Some(ref checksum) = image.checksum {
        basic_fields.push(DetailField::KeyValue {
            key: "Checksum".into(),
            value: checksum.clone(),
            style: None,
        });
    }
    sections.push(DetailSection {
        name: "Basic Info".into(),
        fields: basic_fields,
    });

    // Properties
    let mut prop_fields = vec![
        DetailField::KeyValue {
            key: "Visibility".into(),
            value: image.visibility.clone(),
            style: None,
        },
        DetailField::KeyValue {
            key: "Min Disk".into(),
            value: format!("{} GB", image.min_disk),
            style: None,
        },
        DetailField::KeyValue {
            key: "Min RAM".into(),
            value: format!("{} MB", image.min_ram),
            style: None,
        },
    ];
    if let Some(ref created) = image.created_at {
        prop_fields.push(DetailField::KeyValue {
            key: "Created".into(),
            value: created.clone(),
            style: None,
        });
    }
    sections.push(DetailSection {
        name: "Properties".into(),
        fields: prop_fields,
    });

    DetailData {
        title: format!("Image: {}", image.name),
        sections,
    }
}

pub fn image_create_defs() -> Vec<FieldDef> {
    vec![
        FieldDef::text("Name", true),
        FieldDef::dropdown(
            "Disk Format",
            vec![
                "qcow2".into(),
                "raw".into(),
                "vmdk".into(),
                "vdi".into(),
                "iso".into(),
            ],
            true,
        ),
        FieldDef::dropdown(
            "Container Format",
            vec!["bare".into(), "docker".into(), "ova".into()],
            true,
        ),
        FieldDef::dropdown(
            "Visibility",
            vec![
                "private".into(),
                "public".into(),
                "shared".into(),
                "community".into(),
            ],
            false,
        ),
        FieldDef::text("Min Disk (GB)", false),
        FieldDef::text("Min RAM (MB)", false),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::form::Validation;

    fn make_image() -> Image {
        Image {
            id: "img-1".into(),
            name: "Ubuntu 22.04".into(),
            status: "active".into(),
            disk_format: Some("qcow2".into()),
            container_format: Some("bare".into()),
            size: Some(2_147_483_648),
            visibility: "public".into(),
            min_disk: 10,
            min_ram: 512,
            checksum: Some("abc123".into()),
            created_at: Some("2026-01-01T00:00:00Z".into()),
        }
    }

    #[test]
    fn test_image_columns_count() {
        assert_eq!(image_columns().len(), 5);
    }

    #[test]
    fn test_image_to_row() {
        let img = make_image();
        let row = image_to_row(&img);
        assert_eq!(row.id, "img-1");
        assert_eq!(row.cells[0], "Ubuntu 22.04");
        assert!(row.cells[1].contains("active"));
        assert_eq!(row.cells[2], "qcow2");
        assert_eq!(row.cells[3], "2.0 GB");
        assert_eq!(row.cells[4], "public");
    }

    #[test]
    fn test_image_status_display() {
        assert_eq!(image_status_display("active"), ("●", RowStyleHint::Active));
        assert_eq!(image_status_display("error"), ("✗", RowStyleHint::Error));
        assert_eq!(image_status_display("saving"), ("◐", RowStyleHint::Warning));
        assert_eq!(image_status_display("deactivated"), ("○", RowStyleHint::Disabled));
        assert_eq!(image_status_display("UNKNOWN"), ("?", RowStyleHint::Normal));
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(format_bytes(2_147_483_648), "2.0 GB");
    }

    #[test]
    fn test_image_detail_data() {
        let img = make_image();
        let data = image_detail_data(&img);
        assert_eq!(data.title, "Image: Ubuntu 22.04");
        assert_eq!(data.sections.len(), 2); // Basic + Properties
    }

    #[test]
    fn test_image_create_defs() {
        let defs = image_create_defs();
        assert_eq!(defs.len(), 6);
        assert_eq!(defs[0].name(), "Name");
        assert!(defs[0].validations().contains(&Validation::Required));
        assert_eq!(defs[1].name(), "Disk Format");
        assert!(defs[1].validations().contains(&Validation::Required));
        assert!(matches!(defs[1], FieldDef::Dropdown { .. }));
        assert_eq!(defs[2].name(), "Container Format");
        assert!(defs[2].validations().contains(&Validation::Required));
        assert_eq!(defs[3].name(), "Visibility");
        assert!(!defs[3].validations().contains(&Validation::Required));
        assert_eq!(defs[4].name(), "Min Disk (GB)");
        assert_eq!(defs[5].name(), "Min RAM (MB)");
    }
}
