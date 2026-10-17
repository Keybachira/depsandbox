use crate::app::{DoneInfo, EnvInfo, Msg, StartReq};
use crate::export;
use depsandbox_core::runner::run_shell;
use depsandbox_core::{engine, Adapter, Config};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread;

pub fn spawn_latest(adapter: Arc<dyn Adapter>, dir: PathBuf, tx: Sender<Msg>) {
    thread::spawn(move || {
        let r = adapter.latest_versions(&dir).map_err(|e| format!("{e:#}"));
        let _ = tx.send(Msg::Latest(r));
    });
}

pub fn spawn_versions(adapter: Arc<dyn Adapter>, dir: PathBuf, package: String, tx: Sender<Msg>) {
    thread::spawn(move || {
        let r = adapter.available_versions(&dir, &package).map_err(|e| format!("{e:#}"));
        let _ = tx.send(Msg::Versions(package, r));
    });
}

/// Versões das ferramentas (node, npm...) e o branch git.
pub fn spawn_env(adapter: Arc<dyn Adapter>, dir: PathBuf, tx: Sender<Msg>) {
    thread::spawn(move || {
        let tools = adapter
            .tool_versions_cmds()
            .into_iter()
            .map(|(name, cmd)| {
                let o = run_shell(cmd, &dir);
                (name.to_string(), o.text.lines().next().unwrap_or("?").trim().trim_start_matches('v').to_string())
            })
            .collect();
        let g = run_shell("git rev-parse --abbrev-ref HEAD", &dir);
        let branch = (g.ok && !g.stdout.trim().is_empty()).then(|| g.stdout.trim().to_string());
        let _ = tx.send(Msg::Env(EnvInfo { tools, branch }));
    });
}

pub fn spawn_run(adapter: Arc<dyn Adapter>, dir: PathBuf, cfg: Config, req: StartReq, tx: Sender<Msg>) {
    thread::spawn(move || {
        let ptx = tx.clone();
        let progress = move |m: &str| {
            let _ = ptx.send(Msg::Progress(m.to_string()));
        };
        let res = engine::try_upgrade(adapter.as_ref(), &dir, &req.target, &cfg, &req.opts, &progress)
            .map_err(|e| format!("{e:#}"));
        let msg = match res {
            Ok(report) => {
                let saved = req.save.then(|| export::save_all(&report, &dir).map_err(|e| format!("{e:#}")));
                Msg::Done(Ok(DoneInfo { report: Box::new(report), saved }))
            }
            Err(e) => Msg::Done(Err(e)),
        };
        let _ = tx.send(msg);
    });
}
