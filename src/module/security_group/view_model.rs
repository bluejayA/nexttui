use crate::models::neutron::{SecurityGroup, SecurityGroupRule};
use crate::ui::detail_view::{DetailData, DetailField, DetailSection};
use crate::ui::form::FieldDef;
use crate::ui::resource_list::{ColumnDef, ColumnWidth, Row, RowStyleHint};

pub fn sg_columns(show_tenant: bool) -> Vec<ColumnDef> {
    let mut cols = vec![ColumnDef {
        name: "Name".into(),
        width: ColumnWidth::Percent(30),
        alignment: ratatui::layout::Alignment::Left,
    }];
    if show_tenant {
        cols.push(ColumnDef {
            name: "Project".into(),
            width: ColumnWidth::Percent(12),
            alignment: ratatui::layout::Alignment::Left,
        });
    }
    cols.extend([
        ColumnDef {
            name: "Description".into(),
            width: ColumnWidth::Percent(40),
            alignment: ratatui::layout::Alignment::Left,
        },
        ColumnDef {
            name: "Rules".into(),
            width: ColumnWidth::Fixed(8),
            alignment: ratatui::layout::Alignment::Right,
        },
    ]);
    cols
}

pub fn sg_to_row(sg: &SecurityGroup, show_tenant: bool) -> Row {
    let desc = sg.description.as_deref().unwrap_or("-");
    let rule_count = sg.security_group_rules.len().to_string();
    let mut cells = vec![sg.name.clone()];
    if show_tenant {
        cells.push(sg.tenant_id.as_deref().unwrap_or("-").to_string());
    }
    cells.extend([desc.to_string(), rule_count]);
    Row {
        id: sg.id.clone(),
        cells,
        style_hint: Some(RowStyleHint::Normal),
    }
}

pub fn rule_to_row(rule: &SecurityGroupRule) -> Vec<String> {
    let protocol = rule.protocol.as_deref().unwrap_or("Any");
    let port_range = match (rule.port_range_min, rule.port_range_max) {
        (Some(min), Some(max)) if min == max => min.to_string(),
        (Some(min), Some(max)) => format!("{min}-{max}"),
        _ => "Any".to_string(),
    };
    let source = rule
        .remote_ip_prefix
        .as_deref()
        .or(rule.remote_group_id.as_deref())
        .unwrap_or("Any")
        .to_string();

    vec![
        rule.direction.clone(),
        protocol.to_string(),
        port_range,
        source,
        rule.ethertype.clone(),
    ]
}

pub fn sg_detail_data(sg: &SecurityGroup) -> DetailData {
    let mut sections = vec![];

    // Basic info
    let mut basic_fields = vec![
        DetailField::KeyValue {
            key: "ID".into(),
            value: sg.id.clone(),
            style: None,
        },
        DetailField::KeyValue {
            key: "Name".into(),
            value: sg.name.clone(),
            style: None,
        },
    ];
    if let Some(ref desc) = sg.description {
        basic_fields.push(DetailField::KeyValue {
            key: "Description".into(),
            value: desc.clone(),
            style: None,
        });
    }
    sections.push(DetailSection {
        name: "Basic Info".into(),
        fields: basic_fields,
    });

    // Ingress rules
    let ingress_rules: Vec<&SecurityGroupRule> = sg
        .security_group_rules
        .iter()
        .filter(|r| r.direction.eq_ignore_ascii_case("ingress"))
        .collect();
    if !ingress_rules.is_empty() {
        let columns = vec![
            "Direction".into(),
            "Protocol".into(),
            "Port".into(),
            "Source".into(),
            "Ethertype".into(),
        ];
        let rows: Vec<Vec<String>> = ingress_rules.iter().map(|r| rule_to_row(r)).collect();
        sections.push(DetailSection {
            name: "Ingress Rules".into(),
            fields: vec![DetailField::NestedTable {
                label: "Ingress".into(),
                columns,
                rows,
            }],
        });
    }

    // Egress rules
    let egress_rules: Vec<&SecurityGroupRule> = sg
        .security_group_rules
        .iter()
        .filter(|r| r.direction.eq_ignore_ascii_case("egress"))
        .collect();
    if !egress_rules.is_empty() {
        let columns = vec![
            "Direction".into(),
            "Protocol".into(),
            "Port".into(),
            "Destination".into(),
            "Ethertype".into(),
        ];
        let rows: Vec<Vec<String>> = egress_rules.iter().map(|r| rule_to_row(r)).collect();
        sections.push(DetailSection {
            name: "Egress Rules".into(),
            fields: vec![DetailField::NestedTable {
                label: "Egress".into(),
                columns,
                rows,
            }],
        });
    }

    DetailData {
        title: format!("Security Group: {}", sg.name),
        sections,
    }
}

pub fn sg_create_defs() -> Vec<FieldDef> {
    vec![
        FieldDef::text("Name", true),
        FieldDef::text("Description", false),
    ]
}

pub fn sg_rule_defs() -> Vec<FieldDef> {
    vec![
        FieldDef::dropdown("Direction", vec!["ingress".into(), "egress".into()], true),
        FieldDef::dropdown(
            "Protocol",
            vec!["tcp".into(), "udp".into(), "icmp".into()],
            false,
        ),
        FieldDef::text("Port Min", false),
        FieldDef::text("Port Max", false),
        FieldDef::text("Source CIDR", false),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sg() -> SecurityGroup {
        SecurityGroup {
            id: "sg-1".into(),
            name: "web-sg".into(),
            description: Some("Web security group".into()),
            security_group_rules: vec![
                SecurityGroupRule {
                    id: "rule-1".into(),
                    direction: "ingress".into(),
                    protocol: Some("tcp".into()),
                    port_range_min: Some(80),
                    port_range_max: Some(80),
                    remote_ip_prefix: Some("0.0.0.0/0".into()),
                    remote_group_id: None,
                    ethertype: "IPv4".into(),
                },
                SecurityGroupRule {
                    id: "rule-2".into(),
                    direction: "egress".into(),
                    protocol: None,
                    port_range_min: None,
                    port_range_max: None,
                    remote_ip_prefix: None,
                    remote_group_id: None,
                    ethertype: "IPv4".into(),
                },
            ],
            tenant_id: None,
        }
    }

    #[test]
    fn test_sg_columns_count() {
        assert_eq!(sg_columns(false).len(), 3);
        assert_eq!(sg_columns(true).len(), 4);
        assert_eq!(sg_columns(true)[1].name, "Project");
    }

    #[test]
    fn test_sg_to_row() {
        let sg = make_sg();
        let row = sg_to_row(&sg, false);
        assert_eq!(row.id, "sg-1");
        assert_eq!(row.cells[0], "web-sg");
        assert_eq!(row.cells[1], "Web security group");
        assert_eq!(row.cells[2], "2"); // 2 rules
    }

    #[test]
    fn test_rule_to_row_with_ports() {
        let rule = SecurityGroupRule {
            id: "rule-1".into(),
            direction: "ingress".into(),
            protocol: Some("tcp".into()),
            port_range_min: Some(22),
            port_range_max: Some(22),
            remote_ip_prefix: Some("10.0.0.0/8".into()),
            remote_group_id: None,
            ethertype: "IPv4".into(),
        };
        let row = rule_to_row(&rule);
        assert_eq!(row[0], "ingress");
        assert_eq!(row[1], "tcp");
        assert_eq!(row[2], "22");
        assert_eq!(row[3], "10.0.0.0/8");
    }

    #[test]
    fn test_rule_to_row_port_range() {
        let rule = SecurityGroupRule {
            id: "rule-1".into(),
            direction: "ingress".into(),
            protocol: Some("tcp".into()),
            port_range_min: Some(8000),
            port_range_max: Some(9000),
            remote_ip_prefix: None,
            remote_group_id: None,
            ethertype: "IPv4".into(),
        };
        let row = rule_to_row(&rule);
        assert_eq!(row[2], "8000-9000");
        assert_eq!(row[3], "Any");
    }

    #[test]
    fn test_rule_to_row_any_protocol() {
        let rule = SecurityGroupRule {
            id: "rule-1".into(),
            direction: "egress".into(),
            protocol: None,
            port_range_min: None,
            port_range_max: None,
            remote_ip_prefix: None,
            remote_group_id: None,
            ethertype: "IPv4".into(),
        };
        let row = rule_to_row(&rule);
        assert_eq!(row[1], "Any");
        assert_eq!(row[2], "Any");
    }

    #[test]
    fn test_sg_detail_data() {
        let sg = make_sg();
        let data = sg_detail_data(&sg);
        assert_eq!(data.title, "Security Group: web-sg");
        assert!(data.sections.len() >= 2); // Basic + at least one rule section
        assert!(data.sections.iter().any(|s| s.name == "Ingress Rules"));
        assert!(data.sections.iter().any(|s| s.name == "Egress Rules"));
    }

    #[test]
    fn test_sg_detail_data_no_rules() {
        let sg = SecurityGroup {
            id: "sg-empty".into(),
            name: "empty-sg".into(),
            description: None,
            security_group_rules: vec![],
            tenant_id: None,
        };
        let data = sg_detail_data(&sg);
        assert_eq!(data.sections.len(), 1); // Basic Info only
        assert!(data.sections.iter().all(|s| s.name != "Ingress Rules"));
        assert!(data.sections.iter().all(|s| s.name != "Egress Rules"));
    }

    #[test]
    fn test_sg_create_defs() {
        use crate::ui::form::Validation;
        let defs = sg_create_defs();
        assert_eq!(defs.len(), 2);
        assert_eq!(defs[0].name(), "Name");
        assert!(defs[0].validations().contains(&Validation::Required));
        assert_eq!(defs[1].name(), "Description");
        assert!(!defs[1].validations().contains(&Validation::Required));
    }

    #[test]
    fn test_sg_rule_defs() {
        use crate::ui::form::Validation;
        let defs = sg_rule_defs();
        assert_eq!(defs.len(), 5);
        assert_eq!(defs[0].name(), "Direction");
        assert!(defs[0].validations().contains(&Validation::Required));
        assert!(matches!(defs[0], FieldDef::Dropdown { .. }));
        assert_eq!(defs[1].name(), "Protocol");
        assert!(!defs[1].validations().contains(&Validation::Required));
        assert_eq!(defs[2].name(), "Port Min");
        assert_eq!(defs[3].name(), "Port Max");
        assert_eq!(defs[4].name(), "Source CIDR");
    }
}
