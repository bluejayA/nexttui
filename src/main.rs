use std::io;

use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::mpsc;

use nexttui::app::App;
use nexttui::config::Config;
use nexttui::demo::create_demo_app;
use nexttui::event_loop::run_event_loop;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let demo_mode = args.iter().any(|a| a == "--demo");

    let (mut app, _action_rx, _event_tx) = if demo_mode {
        let (app, action_rx) = create_demo_app();
        let (event_tx, _) = mpsc::unbounded_channel::<nexttui::event::AppEvent>();
        (app, action_rx, event_tx)
    } else {
        // Normal mode: load config, setup channels
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
        let (event_tx, _) = mpsc::unbounded_channel::<nexttui::event::AppEvent>();
        let app = App::new(config, action_tx);
        (app, action_rx, event_tx)
    };

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create event channel for the event loop
    let (_ev_tx, event_rx) = mpsc::unbounded_channel();

    // In demo mode, inject initial data via events was already done in create_demo_app.
    // The event_rx here is just for the event loop to listen on (no background tasks).

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
