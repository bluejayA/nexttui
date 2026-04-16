use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

use nexttui::adapter::auth::keystone::KeystoneAuthAdapter;
use nexttui::adapter::auth::rescope::KeystoneRescopeAdapter;
use nexttui::adapter::auth::scoped_session::ScopedAuthSession;
use nexttui::adapter::auth::token_cache::{self as token_cache, TokenCacheStore};
use nexttui::adapter::http::endpoint_invalidator::EndpointCatalogInvalidator;
use nexttui::adapter::registry::AdapterRegistry;
use nexttui::app::App;
use nexttui::config::Config;
use nexttui::context::{
    CancellationRegistry, ConfigCloudDirectory, ContextHistoryStore, ContextSwitcher,
    ContextTargetResolver, StaticProjectDirectory, SwitchStateMachine,
};
use nexttui::demo::create_demo_app;
use nexttui::event::AppEvent;
use nexttui::event_loop::run_event_loop;
use nexttui::port::auth::AuthProvider;
use nexttui::port::context_session::ContextSessionPort;
use nexttui::port::keystone_rescope::KeystoneRescopePort;
use nexttui::port::scoped_auth::ScopedAuthPort;
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
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("nexttui=info")),
        )
        .with_writer(non_blocking)
        .with_ansi(false)
        .init();

    let args: Vec<String> = std::env::args().collect();
    let demo_mode = args.iter().any(|a| a == "--demo");
    let cloud_arg = args
        .windows(2)
        .find(|w| w[0] == "--cloud")
        .map(|w| w[1].clone());

    // Keep _event_tx alive so event_rx doesn't immediately return None in demo mode
    let (mut app, event_rx, _keep_alive_tx) = if demo_mode {
        let (app, _action_rx) = create_demo_app()?;
        let (event_tx, event_rx) =
            mpsc::unbounded_channel::<nexttui::context::VersionedEvent<AppEvent>>();
        (app, event_rx, Some(event_tx))
    } else {
        let mut config = match Config::load() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error: {e}");
                tracing::error!(%e, "failed to load config");
                std::process::exit(1);
            }
        };

        // --cloud CLI arg overrides OS_CLOUD and config.toml default_cloud
        if let Some(ref name) = cloud_arg
            && let Err(e) = config.switch_cloud(name)
        {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }

        for w in config.warnings() {
            eprintln!("Warning: {w}");
            tracing::warn!(warning = %w, "config warning");
        }

        let current_epoch = Arc::new(nexttui::context::ContextEpoch::new());
        let (action_raw_tx, action_rx) = mpsc::unbounded_channel();
        let action_tx = nexttui::context::ActionSender::new(action_raw_tx, current_epoch.clone());
        let (event_tx, event_rx) = mpsc::unbounded_channel();

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
            project_scope: cloud
                .auth
                .project_name
                .as_ref()
                .map(|pn| ProjectScopeParam {
                    name: pn.clone(),
                    domain_name: cloud
                        .auth
                        .project_domain_name
                        .clone()
                        .unwrap_or_else(|| "Default".to_string()),
                }),
        };

        // === Phase A: capture wire data before config moves ===
        let config_for_wire = Arc::new(config.clone());
        let wire_auth_url = credential.auth_url.clone();
        let wire_username = match &credential.method {
            AuthMethod::Password { username, .. } => username.clone(),
            AuthMethod::ApplicationCredential { id, .. } => id.clone(),
        };

        let auth_provider = Arc::new(KeystoneAuthAdapter::new(credential)?);
        let registry = Arc::new(AdapterRegistry::new_http(
            auth_provider.clone(),
            cloud.region_name.clone(),
        )?);

        // === Phase B: collect endpoint caches before worker consumes registry ===
        let endpoint_caches = registry.endpoint_caches().to_vec();

        // Trigger initial authentication, then initialize RBAC from token roles
        let rbac = std::sync::Arc::new(nexttui::infra::rbac::RbacGuard::new());
        let _ = auth_provider.get_token().await; // force auth before reading roles
        if let Ok(token) = auth_provider.get_token_info().await {
            rbac.update_roles(token.roles, Some(token.project.id));
        }
        let mut module_registry = nexttui::registry::ModuleRegistry::new();
        nexttui::registry::register_all_modules(&mut module_registry, &action_tx);
        let (mut app, initial_actions) =
            App::from_registry(config, action_tx.clone(), module_registry, rbac.clone());

        // Spawn background worker
        tokio::spawn(run_worker(
            registry,
            rbac,
            app.all_tenants.clone(),
            action_rx,
            event_tx.clone(),
        ));

        // Trigger initial data load
        for action in initial_actions {
            let _ = action_tx.send(action);
        }

        // === Phase C: wire context switcher ===
        let cloud_dir = Arc::new(ConfigCloudDirectory::new(config_for_wire.clone()));
        let project_dir = Arc::new(StaticProjectDirectory::new(config_for_wire.clone()));
        let resolver = Arc::new(ContextTargetResolver::new(cloud_dir, project_dir));

        let invalidator = Arc::new(EndpointCatalogInvalidator::new(endpoint_caches));

        let rescope_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()?;
        let rescoper: Arc<dyn KeystoneRescopePort> =
            Arc::new(KeystoneRescopeAdapter::new(rescope_client, wire_auth_url));

        let cloud_key = token_cache::compute_cloud_key(
            &config_for_wire.active_cloud_config().auth.auth_url,
            &wire_username,
        );
        let token_cache = TokenCacheStore::new(token_cache::cache_dir_path(&cloud_key));

        let session: Arc<dyn ContextSessionPort> = Arc::new(ScopedAuthSession::new(
            auth_provider.clone() as Arc<dyn ScopedAuthPort>,
            rescoper,
            invalidator,
            token_cache,
        ));

        let state = Arc::new(SwitchStateMachine::new(current_epoch.clone()));
        let cancellation = Arc::new(CancellationRegistry::new());
        let history = Arc::new(std::sync::Mutex::new(ContextHistoryStore::new()));

        let switcher = Arc::new(ContextSwitcher::new(
            state,
            cancellation,
            resolver,
            session,
            history,
        ));
        app.wire_context_switch(switcher, event_tx);

        (app, event_rx, None)
    };

    // Restore terminal on panic before raw mode corrupts output
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));

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
