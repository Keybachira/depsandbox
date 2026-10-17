//! TUI do depsandbox: lista dependências, testa upgrades em segundo plano e mostra o relatório.
mod app;
mod ui;
mod worker;

use anyhow::{bail, Context, Result};
use app::{App, Cmd};
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use depsandbox_core::{Adapter, Config};
use ratatui::prelude::*;
use std::io::{stdout, IsTerminal};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::time::Duration;

/// Repõe o terminal mesmo se houver pânico ou erro.
struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore();
    }
}

fn restore() {
    let _ = disable_raw_mode();
    let _ = execute!(stdout(), LeaveAlternateScreen);
}

pub fn run(project: PathBuf, adapter: Arc<dyn Adapter>, cfg: Config) -> Result<()> {
    if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
        bail!("a TUI precisa de um terminal interativo");
    }
    let deps = adapter.dependencies(&project)?;
    if deps.is_empty() {
        bail!("nenhuma dependência encontrada em {}", project.display());
    }
    let name = project
        .canonicalize()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "projeto".into());

    let mut app = App::new(name, deps, &cfg.ignore);
    let (tx, rx) = mpsc::channel();
    app.latest_loading = true;
    app.status = "a consultar o registo...".into();
    worker::spawn_latest(adapter.clone(), project.clone(), tx.clone());

    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        hook(info);
    }));

    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    let _guard = TerminalGuard;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    loop {
        app.tick = app.tick.wrapping_add(1);
        terminal.draw(|f| ui::draw(f, &mut app))?;

        while let Ok(m) = rx.try_recv() {
            app.on_msg(m);
        }

        if event::poll(Duration::from_millis(100))? {
            // No Windows o crossterm emite Press e Release: só interessa o Press.
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press {
                    match app.on_key(k) {
                        Some(Cmd::Run(jobs)) => {
                            worker::spawn_runs(adapter.clone(), project.clone(), cfg.clone(), jobs, tx.clone())
                        }
                        Some(Cmd::RefreshLatest) => worker::spawn_latest(adapter.clone(), project.clone(), tx.clone()),
                        Some(Cmd::Save(i)) => app.status = save_report(&app, i, &project),
                        None => {}
                    }
                }
            }
        }
        if app.should_quit {
            break;
        }
    }
    Ok(())
}

fn save_report(app: &App, i: usize, dir: &Path) -> String {
    let Some(rep) = app.report_of(i) else { return "sem relatório para guardar".into() };
    let safe: String = rep
        .target
        .spec()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' })
        .collect();
    let path = dir.join(format!("depsandbox-report-{safe}.md"));
    match std::fs::write(&path, depsandbox_report::to_markdown(rep)).with_context(|| path.display().to_string()) {
        Ok(()) => format!("guardado em {}", path.display()),
        Err(e) => format!("erro ao guardar: {e:#}"),
    }
}
