use crate::app::{App, RowState};
use depsandbox_core::{CheckStatus, FindingKind, Report, Severity, Verdict};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};

const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn draw(f: &mut Frame, app: &mut App) {
    let rows = Layout::vertical([Constraint::Min(3), Constraint::Length(2)]).split(f.size());
    let cols = Layout::horizontal([Constraint::Percentage(42), Constraint::Percentage(58)]).split(rows[0]);
    draw_list(f, app, cols[0]);
    draw_detail(f, app, cols[1]);
    draw_footer(f, app, rows[1]);
}

fn spinner(tick: usize) -> &'static str {
    SPINNER[tick % SPINNER.len()]
}

fn state_icon(state: &RowState, tick: usize) -> Span<'static> {
    match state {
        RowState::Idle => Span::styled("·", Style::new().fg(Color::DarkGray)),
        RowState::Queued => Span::styled("…", Style::new().fg(Color::DarkGray)),
        RowState::Running(_) => Span::styled(spinner(tick), Style::new().fg(Color::Cyan)),
        RowState::Failed(_) => Span::styled("!", Style::new().fg(Color::Red)),
        RowState::Done(r) => match r.verdict {
            Verdict::NoIssuesDetected => Span::styled("✔", Style::new().fg(Color::Green)),
            Verdict::BaselineBroken => Span::styled("⚠", Style::new().fg(Color::Yellow)),
            Verdict::BreakingChangesDetected | Verdict::InstallFailed => {
                Span::styled("✖", Style::new().fg(Color::Red))
            }
        },
    }
}

fn draw_list(f: &mut Frame, app: &mut App, area: Rect) {
    let tick = app.tick;
    let items: Vec<ListItem> = app
        .rows
        .iter()
        .map(|r| {
            let mark = if r.ignored {
                Span::styled("[-]", Style::new().fg(Color::DarkGray))
            } else if r.marked {
                Span::styled("[x]", Style::new().fg(Color::Cyan).bold())
            } else {
                Span::raw("[ ]")
            };
            let mut spans = vec![mark, Span::raw(" "), state_icon(&r.state, tick), Span::raw(" ")];
            let name_style = if r.ignored { Style::new().fg(Color::DarkGray) } else { Style::new() };
            spans.push(Span::styled(r.dep.name.clone(), name_style));
            if r.dep.dev {
                spans.push(Span::styled(" dev", Style::new().fg(Color::DarkGray)));
            }
            spans.push(Span::raw("  "));
            spans.push(Span::styled(r.dep.current.clone(), Style::new().fg(Color::DarkGray)));
            match &r.dep.latest {
                Some(l) => {
                    spans.push(Span::raw(" → "));
                    spans.push(Span::styled(l.clone(), Style::new().fg(Color::Yellow)));
                }
                None if app.latest_loading => spans.push(Span::styled(" …", Style::new().fg(Color::DarkGray))),
                None => {}
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let title = format!(" {} · dependências ", app.project);
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, area, &mut app.list);
}

fn draw_detail(f: &mut Frame, app: &mut App, area: Rect) {
    let tick = app.tick;
    let mut lines: Vec<Line> = vec![];
    let mut title = " detalhe ".to_string();

    if let Some(r) = app.current_row() {
        title = format!(" {} ", r.dep.name);
        let target = match &r.dep.latest {
            Some(l) => format!("{} → {}", r.dep.current, l),
            None => r.dep.current.clone(),
        };
        lines.push(Line::from(Span::styled(target, Style::new().bold())));
        lines.push(Line::raw(""));
        match &r.state {
            RowState::Idle => {
                if r.ignored {
                    lines.push(Line::styled("Ignorado (depsandbox.toml).", Style::new().fg(Color::DarkGray)));
                } else if r.dep.latest.is_none() && !app.latest_loading {
                    lines.push(Line::styled(
                        "Sem versão nova conhecida. Prima t para testar `@latest` mesmo assim.",
                        Style::new().fg(Color::DarkGray),
                    ));
                } else {
                    lines.push(Line::raw("Prima t para testar este upgrade num sandbox."));
                }
            }
            RowState::Queued => lines.push(Line::styled("Na fila…", Style::new().fg(Color::DarkGray))),
            RowState::Running(m) => {
                lines.push(Line::from(vec![Span::styled(spinner(tick), Style::new().fg(Color::Cyan)), Span::raw(format!(" {m}"))]))
            }
            RowState::Failed(e) => {
                lines.push(Line::styled("Erro da ferramenta:", Style::new().fg(Color::Red).bold()));
                lines.extend(e.lines().map(|l| Line::raw(l.to_string())));
            }
            RowState::Done(rep) => report_lines(rep, &mut lines),
        }
    }

    let max = lines.len().saturating_sub(1) as u16;
    app.scroll = app.scroll.min(max);
    let p = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false })
        .scroll((app.scroll, 0));
    f.render_widget(p, area);
}

fn report_lines(rep: &Report, out: &mut Vec<Line<'static>>) {
    let (txt, color) = match rep.verdict {
        Verdict::NoIssuesDetected => ("✔ nenhum problema detectado", Color::Green),
        Verdict::BreakingChangesDetected => ("✖ breaking changes detectadas", Color::Red),
        Verdict::BaselineBroken => ("⚠ baseline já falhava (comparação pouco fiável)", Color::Yellow),
        Verdict::InstallFailed => ("✖ o upgrade não instalou", Color::Red),
    };
    out.push(Line::styled(txt, Style::new().fg(color).bold()));
    out.push(Line::raw(""));

    out.push(Line::styled("Verificações (antes → depois)", Style::new().bold()));
    for b in &rep.baseline {
        let after = rep.after.iter().find(|a| a.name == b.name);
        out.push(Line::from(vec![
            Span::raw(format!("  {:<10}", b.name)),
            status_span(b.status),
            Span::raw("  →  "),
            after.map_or(Span::raw("—"), |a| status_span(a.status)),
        ]));
    }
    if rep.baseline.is_empty() {
        for a in &rep.after {
            out.push(Line::from(vec![Span::raw(format!("  {:<10}— → ", a.name)), status_span(a.status)]));
        }
    }

    out.push(Line::raw(""));
    out.push(Line::styled("Achados", Style::new().bold()));
    if rep.findings.is_empty() {
        out.push(Line::styled("  nenhum", Style::new().fg(Color::DarkGray)));
    }
    let mut findings = rep.findings.clone();
    findings.sort_by(|a, b| b.severity.cmp(&a.severity));
    for fi in &findings {
        let c = match fi.severity {
            Severity::Error => Color::Red,
            Severity::Warning => Color::Yellow,
            Severity::Info => Color::Green,
        };
        let tag = match fi.kind {
            FindingKind::Regression => "regressão",
            FindingKind::Flaky => "instável",
            FindingKind::Fixed => "corrigido",
            FindingKind::PreExistingFailure => "já falhava",
            FindingKind::InstallFailure => "instalação",
        };
        out.push(Line::from(vec![Span::styled(format!("  [{tag}] "), Style::new().fg(c)), Span::raw(fi.message.clone())]));
    }

    for a in rep.after.iter().filter(|a| a.status != CheckStatus::Passed) {
        out.push(Line::raw(""));
        out.push(Line::styled(format!("Saída de `{}` ({})", a.name, a.command), Style::new().bold()));
        for l in a.output_tail.lines() {
            out.push(Line::styled(format!("  {l}"), Style::new().fg(Color::DarkGray)));
        }
    }
}

fn status_span(s: CheckStatus) -> Span<'static> {
    match s {
        CheckStatus::Passed => Span::styled("✔ passou", Style::new().fg(Color::Green)),
        CheckStatus::Failed => Span::styled("✖ falhou", Style::new().fg(Color::Red)),
        CheckStatus::Errored => Span::styled("✖ erro", Style::new().fg(Color::Red)),
        CheckStatus::Flaky => Span::styled("⚠ instável", Style::new().fg(Color::Yellow)),
        CheckStatus::Skipped => Span::styled("– ignorado", Style::new().fg(Color::DarkGray)),
    }
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let keys = " ↑↓ mover · espaço marcar · a todos · t testar · s guardar · r versões · PgUp/PgDn rolar · q sair";
    let status = if app.running || app.latest_loading {
        format!(" {} {}", spinner(app.tick), app.status)
    } else {
        format!(" {}", app.status)
    };
    let lines = vec![
        Line::styled(keys, Style::new().fg(Color::DarkGray)),
        Line::styled(status, Style::new().fg(Color::Cyan)),
    ];
    f.render_widget(Paragraph::new(lines), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use depsandbox_core::Dependency;
    use ratatui::backend::TestBackend;

    #[test]
    fn renders_list_and_detail() {
        let deps = vec![
            Dependency { name: "chalk".into(), current: "4.1.2".into(), latest: Some("5.3.0".into()), dev: false },
            Dependency { name: "vitest".into(), current: "^1.0.0".into(), latest: None, dev: true },
        ];
        let mut app = App::new("demo".into(), deps, &[]);
        let mut term = Terminal::new(TestBackend::new(110, 16)).unwrap();
        term.draw(|f| draw(f, &mut app)).unwrap();
        let buf = term.backend().buffer().clone();
        let text: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(text.contains("chalk"));
        assert!(text.contains("5.3.0"));
        assert!(text.contains("vitest"));
        assert!(text.contains("testar"));
    }
}
