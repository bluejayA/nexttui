use crate::models::cinder::VolumeSnapshot;
use crate::ui::detail_view::{DetailData, DetailField, DetailSection};
use crate::ui::resource_list::{ColumnDef, ColumnWidth, Row, RowStyleHint};

pub fn snapshot_columns() -> Vec<ColumnDef> {
    vec![
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
            name: "Size(GB)".into(),
            width: ColumnWidth::Fixed(9),
            alignment: ratatui::layout::Alignment::Right,
        },
        ColumnDef {
            name: "Volume ID".into(),
            width: ColumnWidth::Percent(25),
            alignment: ratatui::layout::Alignment::Left,
        },
        ColumnDef {
            name: "Created".into(),
            width: ColumnWidth::Percent(20),
            alignment: ratatui::layout::Alignment::Left,
        },
    ]
}

pub fn snapshot_to_row(snap: &VolumeSnapshot) -> Row {
    let (icon, style) = snapshot_status_display(&snap.status);
    let name = snap.name.as_deref().unwrap_or("-");
    let created = snap.created_at.as_deref().unwrap_or("-");
    Row {
        id: snap.id.clone(),
        cells: vec![
            name.to_string(),
            format!("{icon} {}", snap.status),
            snap.size.to_string(),
            snap.volume_id.clone(),
            created.to_string(),
        ],
        style_hint: Some(style),
    }
}

pub fn snapshot_status_display(status: &str) -> (&'static str, RowStyleHint) {
    match status.to_lowercase().as_str() {
        "available" => ("●", RowStyleHint::Active),
        "error" | "error_deleting" => ("✗", RowStyleHint::Error),
        "creating" | "deleting" => ("◐", RowStyleHint::Warning),
        _ => ("?", RowStyleHint::Normal),
    }
}

pub fn snapshot_detail_data(snap: &VolumeSnapshot) -> DetailData {
    let name = snap.name.as_deref().unwrap_or("-");
    let mut fields = vec![
        DetailField::KeyValue {
            key: "ID".into(),
            value: snap.id.clone(),
            style: None,
        },
        DetailField::KeyValue {
            key: "Name".into(),
            value: name.to_string(),
            style: None,
        },
        DetailField::KeyValue {
            key: "Status".into(),
            value: snap.status.clone(),
            style: Some(snapshot_status_display(&snap.status).1),
        },
        DetailField::KeyValue {
            key: "Size".into(),
            value: format!("{} GB", snap.size),
            style: None,
        },
        DetailField::KeyValue {
            key: "Volume ID".into(),
            value: snap.volume_id.clone(),
            style: None,
        },
    ];
    if let Some(ref created) = snap.created_at {
        fields.push(DetailField::KeyValue {
            key: "Created".into(),
            value: created.clone(),
            style: None,
        });
    }

    DetailData {
        title: format!("Snapshot: {}", name),
        sections: vec![DetailSection {
            name: "Basic Info".into(),
            fields,
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_snapshot() -> VolumeSnapshot {
        VolumeSnapshot {
            id: "snap-1".into(),
            name: Some("daily-backup".into()),
            status: "available".into(),
            size: 100,
            volume_id: "vol-1".into(),
            created_at: Some("2026-01-15T00:00:00Z".into()),
        }
    }

    #[test]
    fn test_snapshot_columns_count() {
        assert_eq!(snapshot_columns().len(), 5);
    }

    #[test]
    fn test_snapshot_to_row() {
        let snap = make_snapshot();
        let row = snapshot_to_row(&snap);
        assert_eq!(row.id, "snap-1");
        assert_eq!(row.cells[0], "daily-backup");
        assert!(row.cells[1].contains("available"));
        assert_eq!(row.cells[2], "100");
        assert_eq!(row.cells[3], "vol-1");
        assert_eq!(row.style_hint, Some(RowStyleHint::Active));
    }

    #[test]
    fn test_snapshot_status_display() {
        assert_eq!(snapshot_status_display("available"), ("●", RowStyleHint::Active));
        assert_eq!(snapshot_status_display("error"), ("✗", RowStyleHint::Error));
        assert_eq!(snapshot_status_display("creating"), ("◐", RowStyleHint::Warning));
        assert_eq!(snapshot_status_display("UNKNOWN"), ("?", RowStyleHint::Normal));
    }

    #[test]
    fn test_snapshot_detail_data() {
        let snap = make_snapshot();
        let data = snapshot_detail_data(&snap);
        assert_eq!(data.title, "Snapshot: daily-backup");
        assert_eq!(data.sections.len(), 1);
    }
}
