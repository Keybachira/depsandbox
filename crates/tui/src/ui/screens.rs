//! Telas secundárias: Relatórios, Ambientes, Configurações, Documentação, Sobre.
use super::report_view;
use crate::app::{App, Screen};
use crate::export;
use crate::theme::*;
use depsandbox_core::Verdict;
use ratatui::prelude::*;
use ratatui::widgets::{Paragraph, Wrap};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    match app.screen {
        Screen::Reports => reports(f, app, area),
        Screen::Envs => envs(f, app, area),
        Screen::Settings => settings(f, app, area),
        Screen::Docs => docs(f, area),
        Screen::About => about(f, area),
        Screen::Upgrade => {}
    }
}

fn heading(f: &mut Frame, area: Rect, name: &str, sub: &str) -> Rect {
    let rows = Layout::vertical([Constraint::Length(3), Constraint::Min(3)]).split(area);
    let lines = vec![
        Line::from(vec![dim("depsandbox"), dim("  >  "), Span::styled(name.to_lowercase(), Style::new().fg(ACCENT))]),
        Line::styled(name.to_string(), Style::new().fg(TEXT).bold()),
        Line::styled(sub.to_string(), Style::new().fg(LABEL)),
    ];
    f.render_widget(Paragraph::new(lines), rows[0].inner(&Margin { horizontal: 1, vertical: 0 }));
    rows[1]
}

fn list_line(selected: bool, spans: Vec<Span<'static>>, w: usize) -> Line<'static> {
    let _ = w;
    let mut v = vec![Span::raw(if selected { "▶ " } else { "  " })];
    v.extend(spans);
    let line = Line::from(v);
    if selected { line.style(Style::new().bg(CHIP)) } else { line }
}

fn reports(f: &mut Frame, app: &App, area: Rect) {
    let body = heading(f, area, "Ver Relatórios", "Relatórios guardados em .depsandbox/reports (↑↓ escolhe, PgUp/PgDn rola).");
    let cols = Layout::horizontal([Constraint::Length(46.min(body.width / 2)), Constraint::Min(20)]).split(body);
    let lb = panel("Relatórios");
    let li = lb.inner(cols[0]);
    f.render_widget(lb, cols[0]);
    if app.reports.is_empty() {
        let l = vec![
            Line::styled("Nenhum relatório ainda.", Style::new().fg(DIM)),
            Line::styled("Execute um upgrade com “Gerar relatório”.", Style::new().fg(DIM)),
        ];
        f.render_widget(Paragraph::new(l), li.inner(&Margin { horizontal: 1, vertical: 0 }));
    } else {
        let h = li.height as usize;
        let start = app.reports_cursor.saturating_sub(h.saturating_sub(1));
        let mut lines = vec![];
        for (i, r) in app.reports.iter().enumerate().skip(start).take(h) {
            let (icon, c) = match r.report.verdict {
                Verdict::NoIssuesDetected => ("✔", OK),
                Verdict::BaselineBroken => ("⚠", WARN),
                _ => ("✖", ERR),
            };
            lines.push(list_line(
                i == app.reports_cursor,
                vec![Span::styled(format!("{icon} "), Style::new().fg(c)), text(trunc(&r.report.target.spec(), 22)), dim(format!("  {}", r.stamp.get(5..16).unwrap_or("")))],
                li.width as usize,
            ));
        }
        f.render_widget(Paragraph::new(lines), li);
    }
    let db = panel("Detalhe");
    let di = db.inner(cols[1]).inner(&Margin { horizontal: 1, vertical: 0 });
    f.render_widget(db, cols[1]);
    if let Some(r) = app.reports.get(app.reports_cursor) {
        let lines = report_view::lines(&r.report);
        let max = lines.len().saturating_sub(1) as u16;
        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((app.scroll.min(max), 0)), di);
    }
    let _ = export::reports_dir;
}

fn envs(f: &mut Frame, app: &App, area: Rect) {
    let body = heading(f, area, "Ver Ambientes", "Sandboxes temporários que sobraram (d apaga, r atualiza). Os normais são removidos no fim de cada teste.");
    let b = panel(&format!("Sandboxes em {}", std::env::temp_dir().display()));
    let inner = b.inner(body).inner(&Margin { horizontal: 1, vertical: 0 });
    f.render_widget(b, body);
    if app.envs.is_empty() {
        f.render_widget(Paragraph::new(Line::styled("Nenhum ambiente temporário encontrado. Tudo limpo.", Style::new().fg(OK))), inner);
        return;
    }
    let lines: Vec<Line<'static>> = app
        .envs
        .iter()
        .enumerate()
        .map(|(i, p)| list_line(i == app.envs_cursor, vec![text(trunc_left(&p.display().to_string(), inner.width.saturating_sub(4) as usize))], inner.width as usize))
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}

fn kv(k: &str, v: String) -> Line<'static> {
    Line::from(vec![label(pad_right(k, 18)), text(v)])
}

fn settings(f: &mut Frame, app: &App, area: Rect) {
    let body = heading(f, area, "Configurações", "Lidas de depsandbox.toml (edite o ficheiro para alterar).");
    let b = panel("depsandbox.toml");
    let inner = b.inner(body).inner(&Margin { horizontal: 1, vertical: 0 });
    f.render_widget(b, body);
    let c = &app.cfg;
    let or = |o: &Option<String>| o.clone().unwrap_or_else(|| "(detetado automaticamente)".into());
    let cfg_path = app.project_dir.join(depsandbox_core::config::CONFIG_FILE);
    let lines = vec![
        kv("Ficheiro", format!("{}{}", cfg_path.display(), if cfg_path.exists() { "" } else { "  (não existe: use `depsandbox init`)" })),
        kv("Ecossistema", c.ecosystem.clone().unwrap_or_else(|| "npm".into())),
        kv("Comando de testes", or(&c.checks.test)),
        kv("Comando de build", or(&c.checks.build)),
        kv("Typecheck", or(&c.checks.typecheck)),
        kv("Ignorar", if c.ignore.is_empty() { "—".into() } else { c.ignore.join(", ") }),
        kv("Relatórios em", export::reports_dir(&app.project_dir).display().to_string()),
    ];
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn docs(f: &mut Frame, area: Rect) {
    let body = heading(f, area, "Documentação", "Atalhos e fluxo de uso.");
    let b = panel("Atalhos");
    let inner = b.inner(body).inner(&Margin { horizontal: 1, vertical: 0 });
    f.render_widget(b, body);
    let k = |a: &str, d: &str| Line::from(vec![Span::styled(pad_right(a, 14), Style::new().fg(ACCENT)), text(d.to_string())]);
    let lines = vec![
        Line::styled("Globais", Style::new().fg(TEXT).bold()),
        k("Tab / Shift+Tab", "próxima / anterior tela"),
        k("1 … 6", "ir direto para uma tela"),
        k("Esc", "voltar (fecha detalhes ou volta ao upgrade)"),
        k("q", "sair (pede confirmação durante um teste)"),
        Line::raw(""),
        Line::styled("Executar Upgrade", Style::new().fg(TEXT).bold()),
        k("↑ ↓", "mover entre pacote, versão, opções e botão"),
        k("← →", "mudar versão alvo / alternar coluna de opções"),
        k("Espaço / Enter", "marcar opção · abrir seletor · iniciar"),
        k("Ctrl+K", "selecionar outra dependência (digite para filtrar)"),
        k("t", "iniciar o upgrade de qualquer campo"),
        k("d", "alternar entre etapas e detalhes dos achados"),
        k("s", "guardar relatório (md, json, html)"),
        k("r", "reconsultar versões novas"),
        k("PgUp / PgDn", "rolar detalhes"),
        Line::raw(""),
        Line::styled("O projeto real nunca é modificado: tudo corre numa cópia temporária.", Style::new().fg(DIM)),
    ];
    f.render_widget(Paragraph::new(lines), inner);
}

fn about(f: &mut Frame, area: Rect) {
    let body = heading(f, area, "Sobre", "depsandbox · Dependency Experiment Lab");
    let b = panel("");
    let inner = b.inner(body).inner(&Margin { horizontal: 2, vertical: 1 });
    f.render_widget(b, body);
    let lines = vec![
        Line::from(vec![Span::styled("◆ ", Style::new().fg(ACCENT).bold()), bold("depsandbox"), dim(format!("  v{}", env!("CARGO_PKG_VERSION")))]),
        Line::raw(""),
        Line::from(text("Testa upgrades de dependências num sandbox isolado antes de tocar no projeto real.")),
        Line::from(text("Compara o resultado com um baseline (rodado 2x para detetar testes instáveis)")),
        Line::from(text("e gera um relatório do que quebrou.")),
        Line::raw(""),
        Line::from(label("Ecossistemas: npm (cargo e pip/uv em breve)")),
        Line::from(label("Relatórios: terminal, Markdown, HTML e JSON")),
        Line::raw(""),
        Line::styled("“Passou” significa apenas que nenhuma verificação detetou problema.", Style::new().fg(DIM)),
    ];
    f.render_widget(Paragraph::new(lines), inner);
}
