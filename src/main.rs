use anyhow::Result;
use clap::Parser;
use crossterm::event::{Event, EventStream, KeyCode, KeyModifiers};
use futures::StreamExt;
use std::env;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc::unbounded_channel;
use tokio::time::interval;
use tx::cli::Cli;
use tx::discovery::discover_services;
use tx::mouse::handle_mouse_event;
use tx::pty::{ProcessEvent, PtyProcess};
use tx::state::AppState;
use tx::ui::{install_panic_hook, render_ui, TerminalGuard};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let script = cli.script;

    let target_dir = match cli.dir {
        Some(d) => PathBuf::from(d),
        None => env::current_dir()?,
    };

    let services = match discover_services(&target_dir, &script) {
        Ok(s) if !s.is_empty() => s,
        Ok(_) => {
            eprintln!(
                "tx: No packages found in {} with a '{}' script.",
                target_dir.display(),
                script
            );
            return Ok(());
        }
        Err(e) => {
            eprintln!("tx error: {}", e);
            return Ok(());
        }
    };

    println!(
        "Found {} packages with '{}'. Launching tx TUI...",
        services.len(),
        script
    );

    install_panic_hook();
    let mut terminal = TerminalGuard::init()?;
    let (term_cols, term_rows) = crossterm::terminal::size().unwrap_or((80, 24));

    let term_pane_cols = term_cols.saturating_sub(30).max(20);
    let term_pane_rows = term_rows.saturating_sub(3).max(10);

    let (event_tx, mut event_rx) = unbounded_channel();
    let mut app = AppState::new(services.clone(), script.clone(), term_pane_cols, term_pane_rows);

    let mut processes: Vec<Option<PtyProcess>> = Vec::with_capacity(services.len());

    for (idx, target) in services.iter().enumerate() {
        match PtyProcess::spawn(
            idx,
            &target.command,
            &target.args,
            &target.directory,
            term_pane_cols,
            term_pane_rows,
            event_tx.clone(),
        ) {
            Ok(proc) => {
                processes.push(Some(proc));
                app.set_running(idx);
            }
            Err(_) => {
                processes.push(None);
                app.set_exited(idx, false, Some(1));
            }
        }
    }

    let mut reader = EventStream::new();
    let mut tick = interval(Duration::from_millis(33)); // ~30 fps refresh
    let mut layout_areas = None;

    loop {
        if app.should_quit {
            break;
        }

        tokio::select! {
            _ = tick.tick() => {
                terminal.draw(|f| {
                    let areas = render_ui(f, &app);
                    layout_areas = Some(areas);
                })?;
            }

            Some(process_event) = event_rx.recv() => {
                match process_event {
                    ProcessEvent::Output { service_index, data } => {
                        app.feed_output(service_index, &data);
                    }
                    ProcessEvent::Exited { service_index, success, code } => {
                        app.set_exited(service_index, success, code);
                    }
                }
            }

            Some(Ok(event)) = reader.next() => {
                match event {
                    Event::Key(key) => {
                        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                            app.should_quit = true;
                            break;
                        }

                        match key.code {
                            KeyCode::Char('q') => {
                                app.should_quit = true;
                                break;
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                app.select_prev();
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                app.select_next();
                            }
                            KeyCode::Char('r') => {
                                // Restart active service
                                let active = app.active_index;
                                if let Some(proc) = processes.get_mut(active) {
                                    if let Some(mut p) = proc.take() {
                                        let _ = p.kill();
                                    }
                                    let target = &services[active];
                                    if let Ok(new_proc) = PtyProcess::spawn(
                                        active,
                                        &target.command,
                                        &target.args,
                                        &target.directory,
                                        term_pane_cols,
                                        term_pane_rows,
                                        event_tx.clone(),
                                    ) {
                                        *proc = Some(new_proc);
                                        app.set_running(active);
                                    }
                                }
                            }
                            KeyCode::Char('R') => {
                                // Restart all services
                                for (idx, target) in services.iter().enumerate() {
                                    if let Some(proc) = processes.get_mut(idx) {
                                        if let Some(mut p) = proc.take() {
                                            let _ = p.kill();
                                        }
                                        if let Ok(new_proc) = PtyProcess::spawn(
                                            idx,
                                            &target.command,
                                            &target.args,
                                            &target.directory,
                                            term_pane_cols,
                                            term_pane_rows,
                                            event_tx.clone(),
                                        ) {
                                            *proc = Some(new_proc);
                                            app.set_running(idx);
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }

                    Event::Mouse(mouse_event) => {
                        if let Some(areas) = layout_areas {
                            handle_mouse_event(mouse_event, &areas, &mut app);
                        }
                    }

                    Event::Resize(cols, rows) => {
                        let new_pane_cols = cols.saturating_sub(30).max(20);
                        let new_pane_rows = rows.saturating_sub(3).max(10);
                        for proc_opt in processes.iter_mut() {
                            if let Some(proc) = proc_opt {
                                let _ = proc.resize(new_pane_cols, new_pane_rows);
                            }
                        }
                    }

                    _ => {}
                }
            }
        }
    }

    // Clean up child processes before exiting
    for proc_opt in processes.iter_mut() {
        if let Some(mut proc) = proc_opt.take() {
            let _ = proc.kill();
        }
    }

    TerminalGuard::restore();
    println!("tx stopped. All services terminated.");
    Ok(())
}
