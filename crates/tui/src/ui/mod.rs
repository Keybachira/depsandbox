mod chrome;
mod report_view;
mod right;
mod screens;
mod upgrade;

use crate::app::{App, Screen};
use crate::theme::*;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Paragraph};

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.size();
    f.render_widget(Block::default().style(Style::new().bg(BG).fg(TEXT)), area);
    if area.width < 80 || area.height < 24 {
        let msg = format!("Terminal pequeno ({}x{}). Mínimo: 80x24.", area.width, area.height);
        f.render_widget(Paragraph::new(msg).style(Style::new().fg(WARN)), area);
        return;
    }
    let rows = Layout::vertical([Constraint::Length(3), Constraint::Min(10), Constraint::Length(3)]).split(area);
    chrome::header(f, rows[0]);

    let wide = area.width >= 120;
    let mut content = rows[1];
    if wide {
        let sw = if area.width >= 140 { 28 } else { 22 };
        let cols = Layout::horizontal([Constraint::Length(sw), Constraint::Min(40)]).split(content);
        chrome::sidebar(f, app, cols[0]);
        content = cols[1];
    }
    if app.screen == Screen::Upgrade {
        if area.width >= 90 {
            let rw = ((area.width as usize * 30) / 100).clamp(38, 52) as u16;
            let cols = Layout::horizontal([Constraint::Min(40), Constraint::Length(rw)]).split(content);
            upgrade::draw(f, app, cols[0]);
            right::draw(f, app, cols[1]);
        } else {
            upgrade::draw(f, app, content);
        }
    } else {
        screens::draw(f, app, content);
    }
    chrome::footer(f, app, rows[2]);
    upgrade::draw_picker(f, app, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Msg, RunState};
    use depsandbox_core::*;
    use ratatui::backend::TestBackend;
    use std::path::PathBuf;

    fn dep(name: &str, cur: &str, latest: Option<&str>) -> Dependency {
        Dependency { name: name.into(), current: cur.into(), latest: latest.map(String::from), dev: false }
    }

    fn app() -> App {
        let mut a = App::new(
            "flowty".into(),
            PathBuf::from("."),
            vec![dep("react", "19.1.0", Some("19.2.0")), dep("vitest", "^1.0.0", None)],
            Config::default(),
        );
        a.on_msg(Msg::Latest(Ok([("react".to_string(), "19.2.0".to_string())].into_iter().collect())));
        a
    }

    fn render(a: &App, w: u16, h: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| draw(f, a)).unwrap();
        let buf = term.backend().buffer().clone();
        let mut s = String::new();
        for y in 0..h {
            for x in 0..w {
                s.push_str(buf.get(x, y).symbol());
            }
            s.push('\n');
        }
        s
    }

    fn report() -> Report {
        let cr = |n: &str, st| CheckResult { name: n.into(), command: format!("npm run {n}"), status: st, exit_code: None, duration_ms: 1, output_tail: "boom".into() };
        Report {
            schema_version: 1,
            project: "flowty".into(),
            ecosystem: Ecosystem::Npm,
            target: UpgradeTarget { package: "react".into(), version: Some("19.2.0".into()) },
            manifest: Manifest { depsandbox_version: "0.1.0".into(), os: "linux".into(), arch: "x86_64".into(), tool_versions: vec![], lockfile_sha: None },
            baseline: vec![cr("test", CheckStatus::Passed), cr("typecheck", CheckStatus::Passed)],
            after: vec![cr("test", CheckStatus::Passed), cr("typecheck", CheckStatus::Failed)],
            findings: vec![Finding { kind: FindingKind::Regression, severity: Severity::Error, check: Some("typecheck".into()), message: "'typecheck' passava antes e falha depois.".into(), file: None, line: None }],
            verdict: Verdict::BreakingChangesDetected,
            steps: vec![
                StepTiming { name: "Criando ambiente isolado".into(), duration_ms: 800 },
                StepTiming { name: "Analisando resultados".into(), duration_ms: 2100 },
            ],
        }
    }

    #[test]
    fn idle_screen_has_all_regions() {
        let s = render(&app(), 150, 44);
        for needle in ["depsandbox", "Executar Upgrade", "Ver Relatórios", "Upgrade de Dependência", "Iniciar Upgrade", "Resultados", "Log", "19.2.0", "MODO INTERATIVO"] {
            assert!(s.contains(needle), "faltou {needle:?}\n{s}");
        }
    }

    #[test]
    fn done_screen_shows_results() {
        let mut a = app();
        a.run = RunState::Done { report: Box::new(report()), saved: None };
        let s = render(&a, 150, 44);
        for needle in ["PASSED", "FAILED", "1 item", "Execução concluída", "Tempo total: 2.9s", "100%"] {
            assert!(s.contains(needle), "faltou {needle:?}\n{s}");
        }
        a.show_details = true;
        assert!(render(&a, 150, 44).contains("regressão"));
    }

    #[test]
    fn renders_at_many_sizes_without_panicking() {
        let mut a = app();
        a.run = RunState::Done { report: Box::new(report()), saved: None };
        for (w, h) in [(80, 24), (90, 26), (100, 30), (119, 30), (120, 32), (200, 60)] {
            let _ = render(&a, w, h);
        }
        for s in Screen::ALL {
            a.screen = s;
            let _ = render(&a, 130, 36);
        }
        assert!(render(&a, 60, 20).contains("pequeno"));
    }

    #[test]
    fn picker_overlay_renders() {
        let mut a = app();
        a.on_key(crossterm::event::KeyEvent::new(crossterm::event::KeyCode::Char('k'), crossterm::event::KeyModifiers::CONTROL));
        assert!(render(&a, 130, 36).contains("Selecionar dependência"));
    }
}
