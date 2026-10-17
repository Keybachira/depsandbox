use crate::app::{App, RunState};
use crate::theme::*;
use depsandbox_core::{CheckStatus, FindingKind, Verdict};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let res_h = if area.height >= 34 { 13 } else { 8 };
    let rows = Layout::vertical([
        Constraint::Length(6),
        Constraint::Length(5),
        Constraint::Length(res_h),
        Constraint::Min(0),
    ])
    .split(area);
    project(f, app, rows[0]);
    dependency(f, app, rows[1]);
    results(f, app, rows[2]);
    if rows[3].height >= 4 {
        log(f, app, rows[3]);
    }
}

fn kv(k: &str, v: String) -> Line<'static> {
    Line::from(vec![label(pad_right(k, 10)), text(v)])
}

fn project(f: &mut Frame, app: &App, area: Rect) {
    let b = panel("Projeto");
    let inner = b.inner(area).inner(&Margin { horizontal: 1, vertical: 0 });
    f.render_widget(b, area);
    let w = inner.width as usize;
    let mut first = vec![bold(trunc(&app.project_name, w.saturating_sub(12)))];
    if let Some(br) = app.env.as_ref().and_then(|e| e.branch.clone()) {
        first.push(Span::raw("  "));
        first.push(chip(&format!(" ⎇ {} ", trunc(&br, 14))));
    }
    let mut lines = vec![Line::from(first)];
    match &app.env {
        Some(e) => {
            for (name, v) in &e.tools {
                let nice = if name == "node" { "Node.js" } else { name.as_str() };
                lines.push(kv(nice, v.clone()));
            }
        }
        None => lines.push(Line::styled("a ler ambiente…", Style::new().fg(DIM))),
    }
    let tmp = std::env::temp_dir().join("depsandbox-*").display().to_string();
    lines.push(kv("Sandbox", trunc_left(&tmp, w.saturating_sub(10))));
    f.render_widget(Paragraph::new(lines), inner);
}

fn dependency(f: &mut Frame, app: &App, area: Rect) {
    let b = panel("Dependência");
    let inner = b.inner(area).inner(&Margin { horizontal: 1, vertical: 0 });
    f.render_widget(b, area);
    let Some(d) = app.dep() else { return };
    let target = match app.choice() {
        Some(c) => match &c.version {
            Some(v) if c.is_latest => format!("{v} (latest)"),
            Some(v) => v.clone(),
            None => "latest".to_string(),
        },
        None => "—".to_string(),
    };
    let lines = vec![
        bold(d.name.clone()).into(),
        Line::from(vec![label(pad_right("Atual", 10)), text(d.current.clone())]),
        Line::from(vec![label(pad_right("Alvo", 10)), Span::styled(target, Style::new().fg(ACCENT).bold())]),
    ];
    f.render_widget(Paragraph::new(lines), inner);
}

enum Cell {
    Pending,
    Running,
    Passed,
    Failed,
    Flaky,
    NotRun,
}

fn cell_span(c: &Cell, tick: usize) -> (Span<'static>, Span<'static>) {
    match c {
        Cell::Pending => (Span::styled("·", Style::new().fg(DIM)), dim("—")),
        Cell::Running => (Span::styled(spinner(tick), Style::new().fg(LABEL)), dim("…")),
        Cell::Passed => (Span::styled("✔", Style::new().fg(OK)), Span::styled("PASSED", Style::new().fg(OK))),
        Cell::Failed => (Span::styled("✖", Style::new().fg(ERR)), Span::styled("FAILED", Style::new().fg(ERR).bold())),
        Cell::Flaky => (Span::styled("⚠", Style::new().fg(WARN)), Span::styled("INSTÁVEL", Style::new().fg(WARN))),
        Cell::NotRun => (Span::styled("–", Style::new().fg(DIM)), dim("não executado")),
    }
}

fn results(f: &mut Frame, app: &App, area: Rect) {
    let b = panel("Resultados");
    let inner = b.inner(area).inner(&Margin { horizontal: 1, vertical: 0 });
    f.render_widget(b, area);
    let w = inner.width as usize;

    let check = |name: &str| -> Cell {
        match &app.run {
            RunState::Idle | RunState::Failed(_) => Cell::Pending,
            RunState::Running { .. } => Cell::Running,
            RunState::Done { report, .. } => match report.after.iter().find(|c| c.name == name) {
                Some(c) => match c.status {
                    CheckStatus::Passed => Cell::Passed,
                    CheckStatus::Failed | CheckStatus::Errored => Cell::Failed,
                    CheckStatus::Flaky => Cell::Flaky,
                    CheckStatus::Skipped => Cell::NotRun,
                },
                None => Cell::NotRun,
            },
        }
    };
    let install = match &app.run {
        RunState::Idle | RunState::Failed(_) => Cell::Pending,
        RunState::Running { steps, .. } => if steps.len() >= 3 { Cell::Passed } else { Cell::Running },
        RunState::Done { report, .. } => {
            let broken = report.verdict == Verdict::InstallFailed
                || report.baseline.iter().any(|c| c.name == "install (baseline)");
            if broken { Cell::Failed } else { Cell::Passed }
        }
    };

    let mut lines: Vec<Line<'static>> = vec![];
    let mut row = |lab: &str, c: Cell| {
        let (icon, st) = cell_span(&c, app.tick);
        lines.push(split_line(vec![icon, Span::raw(" "), text(lab.to_string())], vec![st], w));
    };
    row("Instalação", install);
    row("Testes", check("test"));
    row("Build", check("build"));
    row("Verificação de tipos", check("typecheck"));

    // Breaking changes
    let breaking = match &app.run {
        RunState::Done { report, .. } => {
            let n = report
                .findings
                .iter()
                .filter(|x| matches!(x.kind, FindingKind::Regression | FindingKind::InstallFailure))
                .count();
            Some(n)
        }
        _ => None,
    };
    let (icon, val) = match breaking {
        Some(0) => (Span::styled("✔", Style::new().fg(OK)), Span::styled("nenhum", Style::new().fg(OK))),
        Some(n) => (Span::styled("⚠", Style::new().fg(WARN)), Span::styled(format!("{n} {}", if n == 1 { "item" } else { "itens" }), Style::new().fg(WARN).bold())),
        None => (Span::styled("·", Style::new().fg(DIM)), dim("—")),
    };
    lines.push(split_line(vec![icon, Span::raw(" "), text("Breaking changes")], vec![val], w));

    if area.height >= 13 {
        lines.push(Line::raw(""));
        lines.push(Line::styled("Relatório completo", Style::new().fg(TEXT).bold()));
        let (term, html, json) = match &app.run {
            RunState::Done { saved: Some(Ok(s)), .. } => (
                "(d: ver detalhes)".to_string(),
                trunc_left(&s.html, w.saturating_sub(11)),
                trunc_left(&s.json, w.saturating_sub(11)),
            ),
            RunState::Done { saved: None, .. } => ("(d: ver detalhes)".into(), "desligado".into(), "desligado".into()),
            _ => ("—".into(), "—".into(), "—".into()),
        };
        lines.push(kv("Terminal", term));
        lines.push(kv("HTML", html));
        lines.push(kv("JSON", json));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

fn log(f: &mut Frame, app: &App, area: Rect) {
    let b = panel("Log");
    let inner = b.inner(area).inner(&Margin { horizontal: 1, vertical: 0 });
    f.render_widget(b, area);
    let h = inner.height as usize;
    let w = inner.width as usize;
    let start = app.log.len().saturating_sub(h);
    let lines: Vec<Line<'static>> = app.log[start..]
        .iter()
        .map(|(t, m)| {
            let done = m.starts_with("Upgrade concluído");
            Line::from(vec![
                dim(format!("[{t}] ")),
                Span::styled("> ", Style::new().fg(LABEL)),
                Span::styled(trunc(m, w.saturating_sub(13)), Style::new().fg(if done { ACCENT } else { TEXT })),
            ])
        })
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}
