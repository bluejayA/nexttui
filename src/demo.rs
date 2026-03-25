//! Demo mode: generates sample data and wires up all modules without a real API.

use std::collections::HashMap;
use tokio::sync::mpsc;

use crate::action::Action;
use crate::app::App;
use crate::config::Config;
use crate::event::AppEvent;
use crate::models::cinder::{Volume, VolumeAttachment, VolumeSnapshot};
use crate::models::glance::Image;
use crate::models::keystone::Project;
use crate::models::neutron::{FloatingIp, Network, SecurityGroup, SecurityGroupRule};
use crate::models::nova::{
    Address, Flavor, FlavorRef, ImageRef, Server,
};
use crate::registry::{ModuleRegistry, register_all_modules};

/// Create a fully wired App with demo data.
pub fn create_demo_app() -> (App, mpsc::UnboundedReceiver<Action>) {
    let config = demo_config();
    let (action_tx, action_rx) = mpsc::unbounded_channel();

    let mut registry = ModuleRegistry::new();
    register_all_modules(&mut registry, &action_tx);
    let rbac = std::sync::Arc::new(crate::infra::rbac::RbacGuard::new());
    rbac.update_roles(
        vec![crate::port::types::TokenRole { id: "r1".into(), name: "admin".into() }],
        None,
    );
    let (mut app, _initial_actions) = App::from_registry(config, action_tx, registry, rbac);

    // Inject demo data via events
    app.handle_event(AppEvent::ServersLoaded(demo_servers()));
    app.handle_event(AppEvent::FlavorsLoaded(demo_flavors()));
    app.handle_event(AppEvent::NetworksLoaded(demo_networks()));
    app.handle_event(AppEvent::SecurityGroupsLoaded(demo_security_groups()));
    app.handle_event(AppEvent::FloatingIpsLoaded(demo_floating_ips()));
    app.handle_event(AppEvent::VolumesLoaded(demo_volumes()));
    app.handle_event(AppEvent::SnapshotsLoaded(demo_snapshots()));
    app.handle_event(AppEvent::ImagesLoaded(demo_images()));
    app.handle_event(AppEvent::ProjectsLoaded(demo_projects()));

    (app, action_rx)
}

fn demo_config() -> Config {
    let yaml = "clouds:\n  demo-cloud:\n    auth:\n      auth_url: https://keystone.demo.local/v3\n      username: admin\n      password: demo-password\n      project_name: admin\n      user_domain_name: Default\n      project_domain_name: Default\n    region_name: RegionOne\n";

    let path = std::env::temp_dir().join("nexttui-demo-clouds.yaml");
    std::fs::write(&path, yaml).expect("failed to write demo clouds.yaml");
    Config::load_from(&path).expect("failed to load demo config")
}

fn demo_servers() -> Vec<Server> {
    vec![
        Server {
            id: "a1b2c3d4-1111-2222-3333-444455556666".into(),
            name: "web-prod-01".into(),
            status: "ACTIVE".into(),
            addresses: {
                let mut m = HashMap::new();
                m.insert("private-net".into(), vec![
                    Address { addr: "10.0.1.10".into(), version: 4, mac_addr: Some("fa:16:3e:aa:bb:01".into()), ip_type: Some("fixed".into()) },
                    Address { addr: "203.0.113.10".into(), version: 4, mac_addr: None, ip_type: Some("floating".into()) },
                ]);
                m
            },
            flavor: FlavorRef { id: "flv-1".into(), original_name: Some("m1.large".into()), vcpus: Some(4), ram: Some(8192), disk: Some(80) },
            image: Some(ImageRef { id: "img-1".into() }),
            key_name: Some("admin-key".into()),
            availability_zone: Some("az1".into()),
            created: "2026-01-15T09:30:00Z".into(),
            updated: Some("2026-03-20T14:22:00Z".into()),
            tenant_id: Some("proj-admin".into()),
            host_id: None,
            host: Some("compute-01".into()),
        },
        Server {
            id: "b2c3d4e5-2222-3333-4444-555566667777".into(),
            name: "web-prod-02".into(),
            status: "ACTIVE".into(),
            addresses: {
                let mut m = HashMap::new();
                m.insert("private-net".into(), vec![
                    Address { addr: "10.0.1.11".into(), version: 4, mac_addr: Some("fa:16:3e:aa:bb:02".into()), ip_type: Some("fixed".into()) },
                ]);
                m
            },
            flavor: FlavorRef { id: "flv-1".into(), original_name: Some("m1.large".into()), vcpus: Some(4), ram: Some(8192), disk: Some(80) },
            image: Some(ImageRef { id: "img-1".into() }),
            key_name: Some("admin-key".into()),
            availability_zone: Some("az1".into()),
            created: "2026-01-15T09:35:00Z".into(),
            updated: None,
            tenant_id: Some("proj-admin".into()),
            host_id: None,
            host: Some("compute-02".into()),
        },
        Server {
            id: "c3d4e5f6-3333-4444-5555-666677778888".into(),
            name: "db-master".into(),
            status: "ACTIVE".into(),
            addresses: {
                let mut m = HashMap::new();
                m.insert("db-net".into(), vec![
                    Address { addr: "10.0.2.10".into(), version: 4, mac_addr: Some("fa:16:3e:cc:dd:01".into()), ip_type: Some("fixed".into()) },
                ]);
                m
            },
            flavor: FlavorRef { id: "flv-2".into(), original_name: Some("m1.xlarge".into()), vcpus: Some(8), ram: Some(16384), disk: Some(160) },
            image: Some(ImageRef { id: "img-2".into() }),
            key_name: Some("db-key".into()),
            availability_zone: Some("az1".into()),
            created: "2026-02-01T10:00:00Z".into(),
            updated: None,
            tenant_id: Some("proj-admin".into()),
            host_id: None,
            host: Some("compute-01".into()),
        },
        Server {
            id: "d4e5f6a7-4444-5555-6666-777788889999".into(),
            name: "batch-worker-01".into(),
            status: "SHUTOFF".into(),
            addresses: {
                let mut m = HashMap::new();
                m.insert("private-net".into(), vec![
                    Address { addr: "10.0.1.20".into(), version: 4, mac_addr: Some("fa:16:3e:ee:ff:01".into()), ip_type: Some("fixed".into()) },
                ]);
                m
            },
            flavor: FlavorRef { id: "flv-3".into(), original_name: Some("m1.medium".into()), vcpus: Some(2), ram: Some(4096), disk: Some(40) },
            image: Some(ImageRef { id: "img-1".into() }),
            key_name: None,
            availability_zone: Some("az2".into()),
            created: "2026-03-01T08:00:00Z".into(),
            updated: Some("2026-03-20T00:00:00Z".into()),
            tenant_id: Some("proj-admin".into()),
            host_id: None,
            host: Some("compute-03".into()),
        },
        Server {
            id: "e5f6a7b8-5555-6666-7777-888899990000".into(),
            name: "test-broken".into(),
            status: "ERROR".into(),
            addresses: HashMap::new(),
            flavor: FlavorRef { id: "flv-1".into(), original_name: Some("m1.large".into()), vcpus: Some(4), ram: Some(8192), disk: Some(80) },
            image: Some(ImageRef { id: "img-3".into() }),
            key_name: None,
            availability_zone: Some("az1".into()),
            created: "2026-03-22T16:00:00Z".into(),
            updated: None,
            tenant_id: Some("proj-demo".into()),
            host_id: None,
            host: None,
        },
    ]
}

fn demo_flavors() -> Vec<Flavor> {
    vec![
        Flavor { id: "flv-1".into(), name: "m1.large".into(), vcpus: 4, ram: 8192, disk: 80, is_public: true },
        Flavor { id: "flv-2".into(), name: "m1.xlarge".into(), vcpus: 8, ram: 16384, disk: 160, is_public: true },
        Flavor { id: "flv-3".into(), name: "m1.medium".into(), vcpus: 2, ram: 4096, disk: 40, is_public: true },
        Flavor { id: "flv-4".into(), name: "m1.small".into(), vcpus: 1, ram: 2048, disk: 20, is_public: true },
        Flavor { id: "flv-5".into(), name: "c1.2xlarge".into(), vcpus: 16, ram: 32768, disk: 40, is_public: false },
    ]
}

fn demo_networks() -> Vec<Network> {
    vec![
        Network {
            id: "net-priv-1".into(), name: "private-net".into(), status: "ACTIVE".into(),
            description: Some("Default private network".into()),
            admin_state_up: true, external: false, shared: false, mtu: Some(1500),
            port_security_enabled: Some(true), subnets: vec!["sub-1".into()],
            provider_network_type: Some("vxlan".into()), provider_physical_network: None,
            provider_segmentation_id: Some(100),
        },
        Network {
            id: "net-db-1".into(), name: "db-net".into(), status: "ACTIVE".into(),
            description: Some("Database network (isolated)".into()),
            admin_state_up: true, external: false, shared: false, mtu: Some(9000),
            port_security_enabled: Some(true), subnets: vec!["sub-2".into()],
            provider_network_type: Some("vxlan".into()), provider_physical_network: None,
            provider_segmentation_id: Some(200),
        },
        Network {
            id: "net-ext-1".into(), name: "external".into(), status: "ACTIVE".into(),
            description: Some("External floating IP network".into()),
            admin_state_up: true, external: true, shared: true, mtu: Some(1500),
            port_security_enabled: Some(false), subnets: vec!["sub-ext-1".into()],
            provider_network_type: Some("flat".into()), provider_physical_network: Some("physnet1".into()),
            provider_segmentation_id: None,
        },
    ]
}

fn demo_security_groups() -> Vec<SecurityGroup> {
    vec![
        SecurityGroup {
            id: "sg-default".into(), name: "default".into(),
            description: Some("Default security group".into()),
            security_group_rules: vec![
                SecurityGroupRule { id: "r1".into(), direction: "egress".into(), protocol: None, port_range_min: None, port_range_max: None, remote_ip_prefix: None, remote_group_id: None, ethertype: "IPv4".into() },
                SecurityGroupRule { id: "r2".into(), direction: "egress".into(), protocol: None, port_range_min: None, port_range_max: None, remote_ip_prefix: None, remote_group_id: None, ethertype: "IPv6".into() },
            ],
        },
        SecurityGroup {
            id: "sg-web".into(), name: "web-sg".into(),
            description: Some("Web servers - HTTP/HTTPS".into()),
            security_group_rules: vec![
                SecurityGroupRule { id: "r3".into(), direction: "ingress".into(), protocol: Some("tcp".into()), port_range_min: Some(80), port_range_max: Some(80), remote_ip_prefix: Some("0.0.0.0/0".into()), remote_group_id: None, ethertype: "IPv4".into() },
                SecurityGroupRule { id: "r4".into(), direction: "ingress".into(), protocol: Some("tcp".into()), port_range_min: Some(443), port_range_max: Some(443), remote_ip_prefix: Some("0.0.0.0/0".into()), remote_group_id: None, ethertype: "IPv4".into() },
                SecurityGroupRule { id: "r5".into(), direction: "ingress".into(), protocol: Some("tcp".into()), port_range_min: Some(22), port_range_max: Some(22), remote_ip_prefix: Some("10.0.0.0/8".into()), remote_group_id: None, ethertype: "IPv4".into() },
            ],
        },
        SecurityGroup {
            id: "sg-db".into(), name: "db-sg".into(),
            description: Some("Database servers - MySQL/PostgreSQL".into()),
            security_group_rules: vec![
                SecurityGroupRule { id: "r6".into(), direction: "ingress".into(), protocol: Some("tcp".into()), port_range_min: Some(3306), port_range_max: Some(3306), remote_ip_prefix: Some("10.0.1.0/24".into()), remote_group_id: None, ethertype: "IPv4".into() },
                SecurityGroupRule { id: "r7".into(), direction: "ingress".into(), protocol: Some("tcp".into()), port_range_min: Some(5432), port_range_max: Some(5432), remote_ip_prefix: Some("10.0.1.0/24".into()), remote_group_id: None, ethertype: "IPv4".into() },
            ],
        },
    ]
}

fn demo_floating_ips() -> Vec<FloatingIp> {
    vec![
        FloatingIp { id: "fip-1".into(), floating_ip_address: "203.0.113.10".into(), status: "ACTIVE".into(), port_id: Some("port-1".into()), floating_network_id: "net-ext-1".into(), fixed_ip_address: Some("10.0.1.10".into()), router_id: Some("router-1".into()) },
        FloatingIp { id: "fip-2".into(), floating_ip_address: "203.0.113.11".into(), status: "DOWN".into(), port_id: None, floating_network_id: "net-ext-1".into(), fixed_ip_address: None, router_id: None },
        FloatingIp { id: "fip-3".into(), floating_ip_address: "203.0.113.12".into(), status: "ACTIVE".into(), port_id: Some("port-3".into()), floating_network_id: "net-ext-1".into(), fixed_ip_address: Some("10.0.2.10".into()), router_id: Some("router-1".into()) },
    ]
}

fn demo_volumes() -> Vec<Volume> {
    vec![
        Volume { id: "vol-boot-1".into(), name: Some("web-prod-01-boot".into()), description: Some("Boot volume".into()), status: "in-use".into(), size: 80, volume_type: Some("ssd".into()), encrypted: false, bootable: "true".into(), attachments: vec![VolumeAttachment { server_id: "a1b2c3d4-1111-2222-3333-444455556666".into(), device: "/dev/vda".into(), id: "att-1".into() }], availability_zone: Some("az1".into()), created_at: Some("2026-01-15T09:30:00Z".into()) },
        Volume { id: "vol-data-1".into(), name: Some("db-data".into()), description: Some("Database data volume".into()), status: "in-use".into(), size: 500, volume_type: Some("ssd".into()), encrypted: true, bootable: "false".into(), attachments: vec![VolumeAttachment { server_id: "c3d4e5f6-3333-4444-5555-666677778888".into(), device: "/dev/vdb".into(), id: "att-2".into() }], availability_zone: Some("az1".into()), created_at: Some("2026-02-01T10:05:00Z".into()) },
        Volume { id: "vol-backup-1".into(), name: Some("backup-storage".into()), description: None, status: "available".into(), size: 1000, volume_type: Some("hdd".into()), encrypted: false, bootable: "false".into(), attachments: vec![], availability_zone: Some("az2".into()), created_at: Some("2026-03-01T00:00:00Z".into()) },
    ]
}

fn demo_snapshots() -> Vec<VolumeSnapshot> {
    vec![
        VolumeSnapshot { id: "snap-daily-1".into(), name: Some("db-data-daily-0324".into()), status: "available".into(), size: 500, volume_id: "vol-data-1".into(), created_at: Some("2026-03-24T02:00:00Z".into()) },
        VolumeSnapshot { id: "snap-daily-2".into(), name: Some("db-data-daily-0323".into()), status: "available".into(), size: 500, volume_id: "vol-data-1".into(), created_at: Some("2026-03-23T02:00:00Z".into()) },
    ]
}

fn demo_images() -> Vec<Image> {
    vec![
        Image { id: "img-1".into(), name: "Ubuntu 22.04 LTS".into(), status: "active".into(), disk_format: Some("qcow2".into()), container_format: Some("bare".into()), size: Some(2_415_919_104), visibility: "public".into(), min_disk: 10, min_ram: 512, checksum: Some("d4e5f6a7b8c9".into()), created_at: Some("2026-01-01T00:00:00Z".into()) },
        Image { id: "img-2".into(), name: "CentOS 9 Stream".into(), status: "active".into(), disk_format: Some("qcow2".into()), container_format: Some("bare".into()), size: Some(1_073_741_824), visibility: "public".into(), min_disk: 10, min_ram: 512, checksum: Some("a1b2c3d4e5f6".into()), created_at: Some("2026-01-10T00:00:00Z".into()) },
        Image { id: "img-3".into(), name: "Windows Server 2022".into(), status: "active".into(), disk_format: Some("qcow2".into()), container_format: Some("bare".into()), size: Some(16_106_127_360), visibility: "private".into(), min_disk: 40, min_ram: 4096, checksum: Some("f6e5d4c3b2a1".into()), created_at: Some("2026-02-15T00:00:00Z".into()) },
        Image { id: "img-4".into(), name: "Rocky Linux 9".into(), status: "deactivated".into(), disk_format: Some("qcow2".into()), container_format: Some("bare".into()), size: Some(1_610_612_736), visibility: "public".into(), min_disk: 10, min_ram: 512, checksum: None, created_at: Some("2025-12-01T00:00:00Z".into()) },
    ]
}

fn demo_projects() -> Vec<Project> {
    vec![
        Project { id: "proj-admin".into(), name: "admin".into(), description: Some("Admin project".into()), enabled: true, domain_id: Some("default".into()) },
        Project { id: "proj-demo".into(), name: "demo".into(), description: Some("Demo/test project".into()), enabled: true, domain_id: Some("default".into()) },
        Project { id: "proj-staging".into(), name: "staging".into(), description: Some("Staging environment".into()), enabled: true, domain_id: Some("default".into()) },
        Project { id: "proj-old".into(), name: "legacy-app".into(), description: Some("Deprecated project".into()), enabled: false, domain_id: Some("default".into()) },
    ]
}
