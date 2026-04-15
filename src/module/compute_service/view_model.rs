use crate::models::nova::ComputeService;
use crate::ui::resource_list::{ColumnDef, ColumnWidth, Row, RowStyleHint};

pub fn compute_service_columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef {
            name: "Binary".into(),
            width: ColumnWidth::Percent(20),
            alignment: ratatui::layout::Alignment::Left,
        },
        ColumnDef {
            name: "Host".into(),
            width: ColumnWidth::Percent(20),
            alignment: ratatui::layout::Alignment::Left,
        },
        ColumnDef {
            name: "State".into(),
            width: ColumnWidth::Fixed(8),
            alignment: ratatui::layout::Alignment::Left,
        },
        ColumnDef {
            name: "Status".into(),
            width: ColumnWidth::Fixed(10),
            alignment: ratatui::layout::Alignment::Left,
        },
        ColumnDef {
            name: "Disabled Reason".into(),
            width: ColumnWidth::Percent(25),
            alignment: ratatui::layout::Alignment::Left,
        },
    ]
}

pub fn compute_service_to_row(svc: &ComputeService) -> Row {
    let style = match svc.state.as_str() {
        "up" => RowStyleHint::Active,
        "down" => RowStyleHint::Error,
        _ => RowStyleHint::Normal,
    };
    let reason = svc.disabled_reason.as_deref().unwrap_or("-");
    Row {
        id: svc.id.clone(),
        cells: vec![
            svc.binary.clone(),
            svc.host.clone(),
            svc.state.clone(),
            svc.status.clone(),
            reason.to_string(),
        ],
        style_hint: Some(style),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_columns() {
        assert_eq!(compute_service_columns().len(), 5);
    }
    #[test]
    fn test_to_row() {
        let svc = ComputeService {
            id: "s1".into(),
            binary: "nova-compute".into(),
            host: "n1".into(),
            state: "up".into(),
            status: "enabled".into(),
            updated_at: None,
            disabled_reason: None,
        };
        let row = compute_service_to_row(&svc);
        assert_eq!(row.cells[0], "nova-compute");
        assert_eq!(row.cells[4], "-");
        assert_eq!(row.style_hint, Some(RowStyleHint::Active));
    }
}
