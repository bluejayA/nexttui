use crate::models::nova::Flavor;
use crate::ui::form::FormField;
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

pub fn flavor_create_form() -> Vec<FormField> {
    vec![
        FormField::text("Name", true),
        FormField::text("vCPU", true),
        FormField::text("RAM (MB)", true),
        FormField::text("Disk (GB)", true),
        FormField::checkbox("Public"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_flavor_create_form() {
        let form = flavor_create_form();
        assert_eq!(form.len(), 5);
        assert!(form[0].required); // Name
        assert!(form[1].required); // vCPU
        assert!(!form[4].required); // Public checkbox
    }
}
