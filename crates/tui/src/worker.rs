use crate::app::Msg;
use depsandbox_core::{engine, Adapter, Config, UpgradeTarget};
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

/// Corre os upgrades em fila, um sandbox de cada vez.
pub fn spawn_runs(
    adapter: Arc<dyn Adapter>,
    dir: PathBuf,
    cfg: Config,
    jobs: Vec<(usize, UpgradeTarget)>,
    tx: Sender<Msg>,
) {
    thread::spawn(move || {
        for (idx, target) in jobs {
            let ptx = tx.clone();
            let progress = move |m: &str| {
                let _ = ptx.send(Msg::Progress(idx, m.to_string()));
            };
            let r = engine::try_upgrade(adapter.as_ref(), &dir, &target, &cfg, &progress)
                .map_err(|e| format!("{e:#}"));
            let _ = tx.send(Msg::Done(idx, r));
        }
        let _ = tx.send(Msg::Finished);
    });
}
