use std::io::Stdout;
use std::time::Duration;

use crossterm::event::{Event, EventStream};
use futures::StreamExt;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::mpsc;

use crate::app::App;
use crate::error::Result;
use crate::event::AppEvent;

/// Main event loop — runs until App.should_quit becomes true.
pub async fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
    mut event_rx: mpsc::UnboundedReceiver<AppEvent>,
) -> Result<()> {
    let mut key_events = EventStream::new();
    let mut tick = tokio::time::interval(Duration::from_millis(200));

    loop {
        tokio::select! {
            // Branch 1: key input
            key_result = key_events.next() => {
                match key_result {
                    Some(Ok(Event::Key(key))) if key.kind == crossterm::event::KeyEventKind::Press => {
                        app.handle_key(key);
                    }
                    Some(Ok(Event::Resize(_, _))) => {
                        // Resize triggers immediate re-render (handled below)
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) | None => {
                        // Key stream ended or errored — exit gracefully
                        app.should_quit = true;
                    }
                }
            }

            // Branch 2: tick timer
            _ = tick.tick() => {
                app.on_tick();
            }

            // Branch 3: background events
            event = event_rx.recv() => {
                match event {
                    Some(ev) => {
                        app.handle_event(ev);
                    }
                    None => {
                        // All event senders dropped — exit gracefully
                        app.should_quit = true;
                    }
                }
            }
        }

        // Render
        terminal.draw(|f| app.render(f))?;

        // Check quit
        if app.should_quit {
            break;
        }
    }

    Ok(())
}
