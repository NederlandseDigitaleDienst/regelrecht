use crate::app::{App, Tab};
use crate::backend::engine_backend::EngineResponse;
use crate::backend::process_runner::ProcessMessage;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use std::time::Duration;

pub async fn run<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()>
where
    B::Error: Send + Sync + 'static,
{
    loop {
        terminal.draw(|frame| crate::ui::render(frame, app))?;

        if app.should_quit {
            return Ok(());
        }

        // Check for process runner messages (non-blocking)
        while let Some(msg) = app.process_runner.try_recv() {
            dispatch_process_message(app, msg);
        }

        // Check for engine responses (non-blocking)
        while let Some(resp) = app.engine_handle.try_recv() {
            // Forward trace to the trace view
            if let EngineResponse::EvalResult {
                result: _,
                trace: Some(ref trace),
            } = resp
            {
                app.trace.set_trace(trace.clone());
            }
            app.engine.handle_engine_response(resp);
        }

        // Poll for keyboard events with a short timeout for responsiveness
        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
        {
            handle_key(app, key);
        }
    }
}

fn handle_key(app: &mut App, key: KeyEvent) {
    // Ctrl+C always quits
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return;
    }

    // Help overlay toggle
    if app.show_help {
        app.show_help = false;
        return;
    }

    let text_input = is_view_text_input(app);

    // Global keys (only when not in text input mode)
    if !text_input {
        match key.code {
            KeyCode::Char('q') => {
                app.should_quit = true;
                return;
            }
            KeyCode::Char('?') => {
                app.show_help = true;
                return;
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                if let Some(tab) = Tab::from_key(c) {
                    app.active_tab = tab;
                    return;
                }
            }
            KeyCode::BackTab => {
                app.active_tab = app.active_tab.prev();
                return;
            }
            _ => {}
        }
    }

    // Tab key: views with focus switching handle it themselves,
    // otherwise switch to next tab
    if key.code == KeyCode::Tab && !view_handles_tab(app) {
        app.active_tab = app.active_tab.next();
        return;
    }

    // Dispatch to active view
    match app.active_tab {
        Tab::Dashboard => app.dashboard.handle_key(key),
        Tab::Bdd => app.bdd.handle_key(key, &mut app.process_runner),
        Tab::Engine => {
            let engine = &app.engine_handle;
            app.engine.handle_key(key, engine);
        }
        Tab::Corpus => app.corpus.handle_key(key),
        Tab::Pipeline => app.pipeline.handle_key(key),
        Tab::Validation => app.validation.handle_key(key, &mut app.process_runner),
        Tab::Trace => app.trace.handle_key(key),
        Tab::Dependencies => app.deps.handle_key(key),
        Tab::Logs => app.logs.handle_key(key),
        Tab::Actions => app.actions.handle_key(key, &mut app.process_runner),
    }
}

fn dispatch_process_message(app: &mut App, msg: ProcessMessage) {
    match &msg.task_id {
        id if id.starts_with("action:") => app.actions.handle_process_message(msg),
        id if id.starts_with("bdd:") => app.bdd.handle_process_message(msg),
        id if id.starts_with("validate:") => app.validation.handle_process_message(msg),
        _ => {}
    }
}

/// Check if the active view is in a text input mode.
fn is_view_text_input(app: &App) -> bool {
    match app.active_tab {
        Tab::Engine => app.engine.is_text_input(),
        _ => false,
    }
}

/// Check if the active view handles Tab key internally (for pane focus switching).
fn view_handles_tab(app: &App) -> bool {
    matches!(app.active_tab, Tab::Bdd | Tab::Corpus | Tab::Engine)
}
