use crate::models::nova::Aggregate;
use crate::ui::resource_list::{ColumnDef, ColumnWidth, Row, RowStyleHint};

pub fn aggregate_columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef {
            name: "Name".into(),
            width: ColumnWidth::Percent(25),
            alignment: ratatui::layout::Alignment::Left,
        },
        ColumnDef {
            name: "AZ".into(),
            width: ColumnWidth::Percent(15),
            alignment: ratatui::layout::Alignment::Left,
        },
        ColumnDef {
            name: "Hosts".into(),
            width: ColumnWidth::Fixed(8),
            alignment: ratatui::layout::Alignment::Right,
        },
    ]
}

pub fn aggregate_to_row(agg: &Aggregate) -> Row {
    let az = agg.availability_zone.as_deref().unwrap_or("-");
    Row {
        id: agg.id.to_string(),
        cells: vec![
            agg.name.clone(),
            az.to_string(),
            agg.hosts.len().to_string(),
        ],
        style_hint: Some(RowStyleHint::Normal),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_columns() {
        assert_eq!(aggregate_columns().len(), 3);
    }
    #[test]
    fn test_to_row() {
        let agg = Aggregate {
            id: 1,
            name: "agg1".into(),
            availability_zone: Some("az1".into()),
            hosts: vec!["h1".into(), "h2".into()],
            metadata: Default::default(),
        };
        let row = aggregate_to_row(&agg);
        assert_eq!(row.cells[0], "agg1");
        assert_eq!(row.cells[2], "2");
    }
}
