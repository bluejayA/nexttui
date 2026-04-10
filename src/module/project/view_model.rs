use crate::models::keystone::Project;
use crate::ui::detail_view::{DetailData, DetailField, DetailSection};
use crate::ui::form::FieldDef;
use crate::ui::resource_list::{ColumnDef, ColumnWidth, Row, RowStyleHint};

pub fn project_columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef {
            name: "Name".into(),
            width: ColumnWidth::Percent(25),
            alignment: ratatui::layout::Alignment::Left,
        },
        ColumnDef {
            name: "ID".into(),
            width: ColumnWidth::Percent(30),
            alignment: ratatui::layout::Alignment::Left,
        },
        ColumnDef {
            name: "Enabled".into(),
            width: ColumnWidth::Fixed(9),
            alignment: ratatui::layout::Alignment::Center,
        },
        ColumnDef {
            name: "Description".into(),
            width: ColumnWidth::Percent(30),
            alignment: ratatui::layout::Alignment::Left,
        },
    ]
}

pub fn project_to_row(proj: &Project) -> Row {
    let enabled_icon = if proj.enabled { "✓" } else { "✗" };
    let desc = proj.description.as_deref().unwrap_or("-");
    let style = if proj.enabled {
        RowStyleHint::Normal
    } else {
        RowStyleHint::Disabled
    };
    Row {
        id: proj.id.clone(),
        cells: vec![
            proj.name.clone(),
            proj.id.clone(),
            enabled_icon.to_string(),
            desc.to_string(),
        ],
        style_hint: Some(style),
    }
}

pub fn project_detail_data(proj: &Project) -> DetailData {
    let mut fields = vec![
        DetailField::KeyValue {
            key: "ID".into(),
            value: proj.id.clone(),
            style: None,
        },
        DetailField::KeyValue {
            key: "Name".into(),
            value: proj.name.clone(),
            style: None,
        },
        DetailField::KeyValue {
            key: "Enabled".into(),
            value: proj.enabled.to_string(),
            style: None,
        },
    ];
    if let Some(ref desc) = proj.description {
        fields.push(DetailField::KeyValue {
            key: "Description".into(),
            value: desc.clone(),
            style: None,
        });
    }
    if let Some(ref domain) = proj.domain_id {
        fields.push(DetailField::KeyValue {
            key: "Domain ID".into(),
            value: domain.clone(),
            style: None,
        });
    }

    DetailData {
        title: format!("Project: {}", proj.name),
        sections: vec![DetailSection {
            name: "Basic Info".into(),
            fields,
        }],
    }
}

pub fn project_create_defs() -> Vec<FieldDef> {
    vec![
        FieldDef::text("Name", true),
        FieldDef::text("Description", false),
        FieldDef::dropdown("Domain", vec![], true),
        FieldDef::checkbox("Enabled"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_project() -> Project {
        Project {
            id: "proj-1".into(),
            name: "admin".into(),
            description: Some("Admin project".into()),
            enabled: true,
            domain_id: Some("default".into()),
        }
    }

    #[test]
    fn test_project_columns_count() {
        assert_eq!(project_columns().len(), 4);
    }

    #[test]
    fn test_project_to_row() {
        let proj = make_project();
        let row = project_to_row(&proj);
        assert_eq!(row.cells[0], "admin");
        assert_eq!(row.cells[2], "✓");
        assert_eq!(row.style_hint, Some(RowStyleHint::Normal));
    }

    #[test]
    fn test_project_to_row_disabled() {
        let mut proj = make_project();
        proj.enabled = false;
        let row = project_to_row(&proj);
        assert_eq!(row.cells[2], "✗");
        assert_eq!(row.style_hint, Some(RowStyleHint::Disabled));
    }

    #[test]
    fn test_project_detail_data() {
        let proj = make_project();
        let data = project_detail_data(&proj);
        assert_eq!(data.title, "Project: admin");
        assert_eq!(data.sections.len(), 1);
    }

    #[test]
    fn test_project_create_defs() {
        use crate::ui::form::Validation;
        let defs = project_create_defs();
        assert_eq!(defs.len(), 4);
        assert_eq!(defs[0].name(), "Name");
        assert!(defs[0].validations().contains(&Validation::Required));
        assert_eq!(defs[1].name(), "Description");
        assert!(!defs[1].validations().contains(&Validation::Required));
        assert_eq!(defs[2].name(), "Domain");
        assert!(defs[2].validations().contains(&Validation::Required));
        assert!(matches!(defs[2], FieldDef::Dropdown { .. }));
        assert_eq!(defs[3].name(), "Enabled");
        assert!(matches!(defs[3], FieldDef::Checkbox { .. }));
    }
}
