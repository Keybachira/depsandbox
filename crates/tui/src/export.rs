//! Gravação e leitura de relatórios em `<projeto>/.depsandbox/reports/`.
use anyhow::{Context, Result};
use depsandbox_core::Report;
use std::path::{Path, PathBuf};

pub struct Saved {
    pub html: String,
    pub json: String,
    pub md: String,
}

pub struct SavedReport {
    pub stamp: String,
    pub report: Report,
}

pub fn reports_dir(project: &Path) -> PathBuf {
    project.join(".depsandbox").join("reports")
}

fn safe(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' }).collect()
}

/// Grava `.md`, `.json` e `.html`. Devolve os caminhos relativos ao projeto.
pub fn save_all(report: &Report, project: &Path) -> Result<Saved> {
    let dir = reports_dir(project);
    std::fs::create_dir_all(&dir).with_context(|| format!("a criar {}", dir.display()))?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let base = format!("{}-{}", safe(&report.target.spec()), stamp);
    let rel = |ext: &str| format!(".depsandbox/reports/{base}.{ext}");
    let write = |ext: &str, content: String| -> Result<()> {
        let p = project.join(rel(ext));
        std::fs::write(&p, content).with_context(|| format!("a escrever {}", p.display()))
    };
    write("md", depsandbox_report::to_markdown(report))?;
    write("json", depsandbox_report::to_json(report)?)?;
    write("html", depsandbox_report::to_html(report))?;
    Ok(Saved { html: rel("html"), json: rel("json"), md: rel("md") })
}

/// Relatórios guardados, do mais recente para o mais antigo.
pub fn list_reports(project: &Path) -> Vec<SavedReport> {
    let Ok(rd) = std::fs::read_dir(reports_dir(project)) else { return vec![] };
    let mut out: Vec<SavedReport> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |x| x == "json"))
        .filter_map(|e| {
            let stem = e.path().file_stem()?.to_string_lossy().into_owned();
            let report: Report = serde_json::from_str(&std::fs::read_to_string(e.path()).ok()?).ok()?;
            let mut it = stem.rsplitn(3, '-');
            let (time, date) = (it.next()?, it.next()?);
            let stamp = format!(
                "{}-{}-{} {}:{}:{}",
                date.get(0..4)?, date.get(4..6)?, date.get(6..8)?, time.get(0..2)?, time.get(2..4)?, time.get(4..6)?
            );
            Some(SavedReport { stamp, report })
        })
        .collect();
    out.sort_by(|a, b| b.stamp.cmp(&a.stamp));
    out
}
