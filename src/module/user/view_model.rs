use crate::models::keystone::User;
use crate::ui::form::FieldDef;
use crate::ui::resource_list::{ColumnDef, ColumnWidth, Row, RowStyleHint};

pub fn user_columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef {
            name: "Name".into(),
            width: ColumnWidth::Percent(20),
            alignment: ratatui::layout::Alignment::Left,
        },
        ColumnDef {
            name: "Email".into(),
            width: ColumnWidth::Percent(25),
            alignment: ratatui::layout::Alignment::Left,
        },
        ColumnDef {
            name: "Enabled".into(),
            width: ColumnWidth::Fixed(9),
            alignment: ratatui::layout::Alignment::Center,
        },
        ColumnDef {
            name: "Domain ID".into(),
            width: ColumnWidth::Percent(20),
            alignment: ratatui::layout::Alignment::Left,
        },
    ]
}

pub fn user_to_row(user: &User) -> Row {
    let enabled_icon = if user.enabled { "✓" } else { "✗" };
    let email = user.email.as_deref().unwrap_or("-");
    let domain = user.domain_id.as_deref().unwrap_or("-");
    let style = if user.enabled { RowStyleHint::Normal } else { RowStyleHint::Disabled };
    Row {
        id: user.id.clone(),
        cells: vec![user.name.clone(), email.to_string(), enabled_icon.to_string(), domain.to_string()],
        style_hint: Some(style),
    }
}

pub fn user_create_defs() -> Vec<FieldDef> {
    vec![
        FieldDef::text("Username", true),
        FieldDef::text("Password", true),
        FieldDef::text("Email", false),
        FieldDef::text("Domain ID", true),
        FieldDef::checkbox("Enabled"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_user() -> User {
        User {
            id: "user-1".into(),
            name: "admin".into(),
            email: Some("admin@example.com".into()),
            enabled: true,
            default_project_id: None,
            domain_id: Some("default".into()),
        }
    }

    #[test] fn test_user_columns_count() { assert_eq!(user_columns().len(), 4); }
    #[test] fn test_user_to_row() {
        let row = user_to_row(&make_user());
        assert_eq!(row.cells[0], "admin");
        assert_eq!(row.cells[1], "admin@example.com");
        assert_eq!(row.cells[2], "✓");
    }
    #[test] fn test_user_to_row_disabled() {
        let mut u = make_user(); u.enabled = false;
        assert_eq!(user_to_row(&u).style_hint, Some(RowStyleHint::Disabled));
    }
    #[test] fn test_user_create_defs() {
        use crate::ui::form::Validation;
        let defs = user_create_defs();
        assert_eq!(defs.len(), 5);
        assert_eq!(defs[0].name(), "Username");
        assert!(defs[0].validations().contains(&Validation::Required));
        assert_eq!(defs[1].name(), "Password");
        assert!(defs[1].validations().contains(&Validation::Required));
        assert_eq!(defs[2].name(), "Email");
        assert!(!defs[2].validations().contains(&Validation::Required));
        assert_eq!(defs[3].name(), "Domain ID");
        assert!(defs[3].validations().contains(&Validation::Required));
        assert_eq!(defs[4].name(), "Enabled");
        assert!(matches!(defs[4], FieldDef::Checkbox { .. }));
    }
}
