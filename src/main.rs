use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

use nexttui::adapter::auth::keystone::KeystoneAuthAdapter;
use nexttui::adapter::registry::AdapterRegistry;
use nexttui::app::App;
use nexttui::config::Config;
use nexttui::demo::create_demo_app;
use nexttui::event::AppEvent;
use nexttui::event_loop::run_event_loop;
use nexttui::port::auth::AuthProvider;
use nexttui::port::types::{AuthCredential, AuthMethod, ProjectScopeParam};
use nexttui::worker::run_worker;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing (file-based, since TUI owns stdout/stderr)
    let log_dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("nexttui");
    let file_appender = tracing_appender::rolling::daily(&log_dir, "nexttui.log");
    let (non_blocking, _log_guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("nexttui=info")),
        )
        .with_writer(non_blocking)
        .with_ansi(false)
        .init();

    let args: Vec<String> = std::env::args().collect();
    let demo_mode = args.iter().any(|a| a == "--demo");

    // Keep _event_tx alive so event_rx doesn't immediately return None in demo mode
    let (mut app, event_rx, _keep_alive_tx) = if demo_mode {
        let (app, _action_rx) = create_demo_app()?;
        let (event_tx, event_rx) = mpsc::unbounded_channel::<AppEvent>();
        (app, event_rx, Some(event_tx))
    } else {
        let config = match Config::load() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error: {e}");
                tracing::error!(%e, "failed to load config");
                std::process::exit(1);
            }
        };

        for w in config.warnings() {
            eprintln!("Warning: {w}");
            tracing::warn!(warning = %w, "config warning");
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

        let auth_provider = Arc::new(KeystoneAuthAdapter::new(credential)?);
        let registry = Arc::new(AdapterRegistry::new_http(
            auth_provider.clone(),
            cloud.region_name.clone(),
        )?);

        // Initialize RBAC from token roles
        let rbac = std::sync::Arc::new(nexttui::infra::rbac::RbacGuard::new());
        if let Ok(token) = auth_provider.get_token_info().await {
            rbac.update_roles(token.roles, Some(token.project.id));
        }
        let mut module_registry = nexttui::registry::ModuleRegistry::new();
        nexttui::registry::register_all_modules(&mut module_registry, &action_tx);
        let (app, initial_actions) = App::from_registry(config, action_tx.clone(), module_registry, rbac.clone());

        // Spawn background worker
        tokio::spawn(run_worker(registry, rbac, app.all_tenants.clone(), action_rx, event_tx));

        // Trigger initial data load
        for action in initial_actions {
            let _ = action_tx.send(action);
        }

        (app, event_rx, None)
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
        tracing::error!(%e, "event loop error");
        std::process::exit(1);
    }

    Ok(())
}
