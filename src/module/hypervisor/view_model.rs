use crate::models::nova::Hypervisor;
use crate::ui::resource_list::{ColumnDef, ColumnWidth, Row, RowStyleHint};

pub fn hypervisor_columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef { name: "Hostname".into(), width: ColumnWidth::Percent(20), alignment: ratatui::layout::Alignment::Left },
        ColumnDef { name: "State".into(), width: ColumnWidth::Fixed(8), alignment: ratatui::layout::Alignment::Left },
        ColumnDef { name: "Status".into(), width: ColumnWidth::Fixed(10), alignment: ratatui::layout::Alignment::Left },
        ColumnDef { name: "vCPUs".into(), width: ColumnWidth::Fixed(12), alignment: ratatui::layout::Alignment::Right },
        ColumnDef { name: "RAM (MB)".into(), width: ColumnWidth::Fixed(16), alignment: ratatui::layout::Alignment::Right },
        ColumnDef { name: "VMs".into(), width: ColumnWidth::Fixed(6), alignment: ratatui::layout::Alignment::Right },
    ]
}

pub fn hypervisor_to_row(hv: &Hypervisor) -> Row {
    let state_style = match hv.state.as_str() {
        "up" => RowStyleHint::Active,
        "down" => RowStyleHint::Error,
        _ => RowStyleHint::Normal,
    };
    Row {
        id: hv.id.clone(),
        cells: vec![
            hv.hypervisor_hostname.clone(),
            hv.state.clone(),
            hv.status.clone(),
            format!("{}/{}", hv.vcpus_used, hv.vcpus),
            format!("{}/{}", hv.memory_mb_used, hv.memory_mb),
            hv.running_vms.to_string(),
        ],
        style_hint: Some(state_style),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test] fn test_columns() { assert_eq!(hypervisor_columns().len(), 6); }
    #[test] fn test_to_row() {
        let hv = Hypervisor {
            id: "h1".into(), hypervisor_hostname: "node1".into(), state: "up".into(),
            status: "enabled".into(), vcpus: 64, vcpus_used: 32,
            memory_mb: 131072, memory_mb_used: 65536, running_vms: 10,
            hypervisor_type: "QEMU".into(), local_gb: 1000, local_gb_used: 500,
        };
        let row = hypervisor_to_row(&hv);
        assert_eq!(row.cells[0], "node1");
        assert_eq!(row.cells[3], "32/64");
        assert_eq!(row.style_hint, Some(RowStyleHint::Active));
    }
}
