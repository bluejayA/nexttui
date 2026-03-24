use crate::models::nova::Flavor;
use crate::ui::form::FieldDef;
use crate::ui::resource_list::{ColumnDef, ColumnWidth, Row, RowStyleHint};

pub fn flavor_columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef {
            name: "Name".into(),
            width: ColumnWidth::Percent(25),
            alignment: ratatui::layout::Alignment::Left,
        },
        ColumnDef {
            name: "vCPU".into(),
            width: ColumnWidth::Fixed(8),
            alignment: ratatui::layout::Alignment::Right,
        },
        ColumnDef {
            name: "RAM (MB)".into(),
            width: ColumnWidth::Fixed(10),
            alignment: ratatui::layout::Alignment::Right,
        },
        ColumnDef {
            name: "Disk (GB)".into(),
            width: ColumnWidth::Fixed(10),
            alignment: ratatui::layout::Alignment::Right,
        },
        ColumnDef {
            name: "Public".into(),
            width: ColumnWidth::Fixed(8),
            alignment: ratatui::layout::Alignment::Center,
        },
    ]
}

pub fn flavor_to_row(flavor: &Flavor) -> Row {
    let public_icon = if flavor.is_public { "✓" } else { "✗" };
    Row {
        id: flavor.id.clone(),
        cells: vec![
            flavor.name.clone(),
            flavor.vcpus.to_string(),
            flavor.ram.to_string(),
            flavor.disk.to_string(),
            public_icon.to_string(),
        ],
        style_hint: Some(RowStyleHint::Normal),
    }
}

pub fn flavor_create_defs() -> Vec<FieldDef> {
    vec![
        FieldDef::text("Name", true),
        FieldDef::text("vCPU", true),
        FieldDef::text("RAM (MB)", true),
        FieldDef::text("Disk (GB)", true),
        FieldDef::checkbox("Public"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::form::Validation;

    fn make_flavor() -> Flavor {
        Flavor {
            id: "flv-1".into(),
            name: "m1.small".into(),
            vcpus: 2,
            ram: 4096,
            disk: 40,
            is_public: true,
        }
    }

    #[test]
    fn test_flavor_columns_count() {
        assert_eq!(flavor_columns().len(), 5);
    }

    #[test]
    fn test_flavor_to_row() {
        let flavor = make_flavor();
        let row = flavor_to_row(&flavor);
        assert_eq!(row.id, "flv-1");
        assert_eq!(row.cells[0], "m1.small");
        assert_eq!(row.cells[1], "2");
        assert_eq!(row.cells[2], "4096");
        assert_eq!(row.cells[3], "40");
        assert_eq!(row.cells[4], "✓");
    }

    #[test]
    fn test_flavor_create_defs() {
        let defs = flavor_create_defs();
        assert_eq!(defs.len(), 5);
        assert_eq!(defs[0].name(), "Name");
        assert!(defs[0].validations().contains(&Validation::Required));
        assert_eq!(defs[1].name(), "vCPU");
        assert!(defs[1].validations().contains(&Validation::Required));
        assert_eq!(defs[2].name(), "RAM (MB)");
        assert!(defs[2].validations().contains(&Validation::Required));
        assert_eq!(defs[3].name(), "Disk (GB)");
        assert!(defs[3].validations().contains(&Validation::Required));
        assert_eq!(defs[4].name(), "Public");
        assert!(matches!(defs[4], FieldDef::Checkbox { .. }));
    }
}
