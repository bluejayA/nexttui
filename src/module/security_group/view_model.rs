use crate::models::neutron::{SecurityGroup, SecurityGroupRule};
use crate::ui::detail_view::{DetailData, DetailField, DetailSection};
use crate::ui::form::FormField;
use crate::ui::resource_list::{ColumnDef, ColumnWidth, Row, RowStyleHint};

pub fn sg_columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef {
            name: "Name".into(),
            width: ColumnWidth::Percent(30),
            alignment: ratatui::layout::Alignment::Left,
        },
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
    ]
}

pub fn sg_to_row(sg: &SecurityGroup) -> Row {
    let desc = sg.description.as_deref().unwrap_or("-");
    let rule_count = sg.security_group_rules.len().to_string();
    Row {
        id: sg.id.clone(),
        cells: vec![sg.name.clone(), desc.to_string(), rule_count],
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

pub fn sg_create_form() -> Vec<FormField> {
    vec![
        FormField::text("Name", true),
        FormField::text("Description", false),
    ]
}

pub fn sg_rule_form() -> Vec<FormField> {
    vec![
        FormField::dropdown("Direction", vec!["ingress".into(), "egress".into()], true),
        FormField::dropdown(
            "Protocol",
            vec!["tcp".into(), "udp".into(), "icmp".into()],
            false,
        ),
        FormField::text("Port Min", false),
        FormField::text("Port Max", false),
        FormField::text("Source CIDR", false),
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
        }
    }

    #[test]
    fn test_sg_columns_count() {
        assert_eq!(sg_columns().len(), 3);
    }

    #[test]
    fn test_sg_to_row() {
        let sg = make_sg();
        let row = sg_to_row(&sg);
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
        };
        let data = sg_detail_data(&sg);
        assert_eq!(data.sections.len(), 1); // Basic Info only
        assert!(data.sections.iter().all(|s| s.name != "Ingress Rules"));
        assert!(data.sections.iter().all(|s| s.name != "Egress Rules"));
    }

    #[test]
    fn test_sg_create_form() {
        let form = sg_create_form();
        assert_eq!(form.len(), 2);
        assert!(form[0].required);
        assert!(!form[1].required);
    }

    #[test]
    fn test_sg_rule_form() {
        let form = sg_rule_form();
        assert_eq!(form.len(), 5);
        assert!(form[0].required); // Direction
        assert!(!form[1].required); // Protocol
    }
}
