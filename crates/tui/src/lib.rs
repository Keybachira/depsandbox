//! TUI do depsandbox: assistente de upgrade, execução em segundo plano, resultados e relatórios.
mod app;
mod export;
mod theme;
mod ui;
mod worker;

use anyhow::{bail, Result};
use app::{App, Cmd, Msg};
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use depsandbox_core::{Adapter, Config};
use ratatui::prelude::*;
use std::io::{stdout, IsTerminal};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
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

    let mut app = App::new(name, project.clone(), deps, cfg.clone());
    let (tx, rx) = mpsc::channel::<Msg>();
    app.latest_loading = true;
    app.status = "a consultar o registo...".into();
    app.log_push("Sessão iniciada");
    worker::spawn_latest(adapter.clone(), project.clone(), tx.clone());
    worker::spawn_env(adapter.clone(), project.clone(), tx.clone());
    worker::spawn_versions(adapter.clone(), project.clone(), app.deps[0].name.clone(), tx.clone());

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
        terminal.draw(|f| ui::draw(f, &app))?;

        while let Ok(m) = rx.try_recv() {
            if let Some(cmd) = app.on_msg(m) {
                exec(cmd, &mut app, &adapter, &project, &cfg, &tx);
            }
        }

        if event::poll(Duration::from_millis(100))? {
            // No Windows o crossterm emite Press e Release: só interessa o Press.
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press {
                    if let Some(cmd) = app.on_key(k) {
                        exec(cmd, &mut app, &adapter, &project, &cfg, &tx);
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

fn exec(cmd: Cmd, app: &mut App, adapter: &Arc<dyn Adapter>, project: &Path, cfg: &Config, tx: &Sender<Msg>) {
    match cmd {
        Cmd::Start(req) => worker::spawn_run(adapter.clone(), project.to_path_buf(), cfg.clone(), req, tx.clone()),
        Cmd::LoadVersions(name) => worker::spawn_versions(adapter.clone(), project.to_path_buf(), name, tx.clone()),
        Cmd::RefreshLatest => {
            worker::spawn_latest(adapter.clone(), project.to_path_buf(), tx.clone());
            if let Some(d) = app.dep() {
                worker::spawn_versions(adapter.clone(), project.to_path_buf(), d.name.clone(), tx.clone());
            }
        }
        Cmd::SaveNow => {
            let res = app.report().map(|r| export::save_all(r, project));
            app.status = match res {
                Some(Ok(s)) => format!("guardado em {}", s.json.trim_end_matches(".json")),
                Some(Err(e)) => format!("erro ao guardar: {e:#}"),
                None => "sem relatório para guardar".into(),
            };
            let st = app.status.clone();
            app.log_push(st);
        }
    }
}
