use std::io;
use std::sync::Arc;

use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::mpsc;

use nexttui::adapter::auth::keystone::KeystoneAuthAdapter;
use nexttui::adapter::registry::AdapterRegistry;
use nexttui::app::App;
use nexttui::config::Config;
use nexttui::demo::create_demo_app;
use nexttui::event::AppEvent;
use nexttui::event_loop::run_event_loop;
use nexttui::port::types::{AuthCredential, AuthMethod, ProjectScopeParam};
use nexttui::worker::run_worker;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let demo_mode = args.iter().any(|a| a == "--demo");

    let (mut app, event_rx) = if demo_mode {
        let (app, _action_rx) = create_demo_app();
        let (_event_tx, event_rx) = mpsc::unbounded_channel::<AppEvent>();
        (app, event_rx)
    } else {
        let config = match Config::load() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        };

        for w in config.warnings() {
            eprintln!("Warning: {w}");
        }

        let (action_tx, action_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel::<AppEvent>();

        // Build auth credential from config
        let cloud = config.active_cloud_config();
        let credential = AuthCredential {
            auth_url: cloud.auth.auth_url.clone(),
            method: AuthMethod::Password {
                username: cloud.auth.username.clone().unwrap_or_default(),
                password: cloud.auth.password.clone().unwrap_or_default(),
                domain_name: cloud
                    .auth
                    .user_domain_name
                    .clone()
                    .unwrap_or_else(|| "Default".to_string()),
            },
            project_scope: cloud.auth.project_name.as_ref().map(|pn| ProjectScopeParam {
                name: pn.clone(),
                domain_name: cloud
                    .auth
                    .project_domain_name
                    .clone()
                    .unwrap_or_else(|| "Default".to_string()),
            }),
        };

        let auth_provider = Arc::new(KeystoneAuthAdapter::new(credential));
        let registry = Arc::new(AdapterRegistry::new_http(
            auth_provider,
            cloud.region_name.clone(),
        ));

        // Spawn background worker
        tokio::spawn(run_worker(registry, action_rx, event_tx));

        let mut app = App::new(config, action_tx.clone());

        // Register all modules
        use nexttui::models::common::Route;
        use nexttui::module::{
            server::ServerModule, flavor::FlavorModule, network::NetworkModule,
            security_group::SecurityGroupModule, floating_ip::FloatingIpModule,
            volume::VolumeModule, snapshot::SnapshotModule, image::ImageModule,
            project::ProjectModule, user::UserModule,
        };
        app.register_component(Route::Servers, Box::new(ServerModule::new(action_tx.clone())));
        app.register_component(Route::Flavors, Box::new(FlavorModule::new(action_tx.clone(), true)));
        app.register_component(Route::Networks, Box::new(NetworkModule::new(action_tx.clone())));
        app.register_component(Route::SecurityGroups, Box::new(SecurityGroupModule::new(action_tx.clone())));
        app.register_component(Route::FloatingIps, Box::new(FloatingIpModule::new(action_tx.clone())));
        app.register_component(Route::Volumes, Box::new(VolumeModule::new(action_tx.clone())));
        app.register_component(Route::Snapshots, Box::new(SnapshotModule::new(action_tx.clone())));
        app.register_component(Route::Images, Box::new(ImageModule::new(action_tx.clone(), true)));
        app.register_component(Route::Projects, Box::new(ProjectModule::new(action_tx.clone())));
        app.register_component(Route::Users, Box::new(UserModule::new(action_tx.clone())));

        // Trigger initial data load
        let _ = action_tx.send(nexttui::action::Action::FetchServers);
        let _ = action_tx.send(nexttui::action::Action::FetchFlavors);
        let _ = action_tx.send(nexttui::action::Action::FetchNetworks);
        let _ = action_tx.send(nexttui::action::Action::FetchSecurityGroups);
        let _ = action_tx.send(nexttui::action::Action::FetchFloatingIps);
        let _ = action_tx.send(nexttui::action::Action::FetchVolumes);
        let _ = action_tx.send(nexttui::action::Action::FetchSnapshots);
        let _ = action_tx.send(nexttui::action::Action::FetchImages);
        let _ = action_tx.send(nexttui::action::Action::FetchProjects);
        let _ = action_tx.send(nexttui::action::Action::FetchUsers);

        (app, event_rx)
    };

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_event_loop(&mut terminal, &mut app, event_rx).await;

    // Cleanup terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }

    Ok(())
}
