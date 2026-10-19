use super::report_view;
use crate::app::{App, Focus, RunState};
use crate::theme::*;
use depsandbox_core::Verdict;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};

/// Etapas que o motor percorre num upgrade completo (para a barra de progresso).
const EXPECTED_STEPS: usize = 6;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let roomy = area.height >= 34;
    let cfg_h = if roomy { 11 } else { 8 };
    let rows = Layout::vertical([Constraint::Length(3), Constraint::Length(cfg_h), Constraint::Min(5)]).split(area);
    title(f, rows[0]);
    config(f, app, rows[1], roomy);
    execution(f, app, rows[2]);
}

fn title(f: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from(vec![dim("depsandbox"), dim("  >  "), Span::styled("upgrade", Style::new().fg(ACCENT))]),
        Line::styled("Upgrade de Dependência", Style::new().fg(TEXT).bold()),
        Line::styled(
            if area.width >= 84 {
                "Teste num ambiente isolado e veja o relatório de breaking changes."
            } else {
                "Teste num sandbox e veja os breaking changes."
            },
            Style::new().fg(LABEL),
        ),
    ];
    f.render_widget(Paragraph::new(lines), area.inner(&Margin { horizontal: 1, vertical: 0 }));
}

fn field(t: String, focused: bool) -> Span<'static> {
    if focused {
        Span::styled(t, Style::new().fg(BG).bg(ACCENT).bold())
    } else {
        Span::styled(t, Style::new().fg(TEXT).bg(CHIP))
    }
}

fn opt(app: &App, i: usize, name: &str, colw: usize) -> Vec<Span<'static>> {
    let on = app.opts[i];
    let focused = app.focus == Focus::Opt(i);
    let t = pad_right(&format!("[{}] {}", if on { "x" } else { " " }, name), colw);
    if focused {
        vec![Span::styled(t, Style::new().fg(BG).bg(ACCENT).bold())]
    } else {
        let (b, rest) = t.split_at(3);
        vec![
            Span::styled(b.to_string(), Style::new().fg(if on { ACCENT } else { DIM })),
            Span::styled(rest.to_string(), Style::new().fg(TEXT)),
        ]
    }
}

fn config(f: &mut Frame, app: &App, area: Rect, roomy: bool) {
    let block = panel("");
    let inner = block.inner(area).inner(&Margin { horizontal: 1, vertical: 0 });
    f.render_widget(block, area);
    let w = inner.width as usize;
    let gap = |l: &mut Vec<Line<'static>>| {
        if roomy {
            l.push(Line::raw(""));
        }
    };
    let ind = " ".repeat(4);
    let mut lines: Vec<Line<'static>> = vec![];

    // 1 · Projeto
    let path = app.project_dir.canonicalize().unwrap_or_else(|_| app.project_dir.clone());
    let path = path.display().to_string();
    let path = path.strip_prefix(r"\\?\").unwrap_or(&path).to_string();
    let head_w = 4 + 12 + app.project_name.chars().count() + 14;
    lines.push(Line::from(vec![
        chip(" 1 "),
        Span::raw(" "),
        bold(pad_right("Projeto", 12)),
        Span::styled(app.project_name.clone(), Style::new().fg(TEXT).bg(CHIP)),
        dim("  ·  "),
        label("npm"),
        dim("  ·  "),
        dim(trunc_left(&path, w.saturating_sub(head_w))),
    ]));
    gap(&mut lines);

    // 2 · Pacote
    let dep = app.dep();
    let (name, cur) = dep.map_or(("—".to_string(), "—".to_string()), |d| (d.name.clone(), d.current.clone()));
    lines.push(Line::from(vec![
        chip(" 2 "),
        Span::raw(" "),
        bold(pad_right("Dependência", 12)),
        field(format!(" {name} "), app.focus == Focus::Pkg),
        label("   atual "),
        text(cur),
        if app.latest_loading { dim("   a consultar…") } else { Span::raw("") },
    ]));
    let ver_label = match app.choice() {
        Some(c) => c.label.clone(),
        None => "—".into(),
    };
    let more = if app.choices_loading { " …" } else { "" };
    lines.push(Line::from(vec![
        Span::raw(ind.to_string()),
        label(pad_right("Versão alvo", 12)),
        field(format!(" {ver_label} ▾{more} "), app.focus == Focus::Ver),
        if w >= 62 { dim("   Selecionar outra ") } else { Span::raw("") },
        if w >= 62 { chip(" Ctrl+K ") } else { Span::raw("") },
    ]));
    gap(&mut lines);

    // 3 · Opções (rótulos curtos quando o centro é estreito)
    let full = w >= 64;
    let names = if full {
        ["Executar testes", "Verificação de tipos", "Executar build", "Gerar relatório"]
    } else {
        ["Executar testes", "Verificar tipos", "Executar build", "Gerar relatório"]
    };
    let colw = if full { (w.saturating_sub(16) / 2).min(28) } else { (w.saturating_sub(16) / 2).max(16) };
    let mut l1 = vec![chip(" 3 "), Span::raw(" "), bold(pad_right("Opções", 12))];
    l1.extend(opt(app, 0, names[0], colw));
    l1.extend(opt(app, 1, names[1], colw));
    lines.push(Line::from(l1));
    let mut l2 = vec![Span::raw(format!("{ind}{}", " ".repeat(12)))];
    l2.extend(opt(app, 2, names[2], colw));
    l2.extend(opt(app, 3, names[3], colw));
    lines.push(Line::from(l2));
    gap(&mut lines);

    // Botão
    let bw = w.saturating_sub(1);
    let focused = app.focus == Focus::Start;
    let (txt, bg) = if app.running() {
        (format!("{}  Executando…", spinner(app.tick)), ACCENT_DIM)
    } else {
        ("▶  Iniciar Upgrade".to_string(), if focused { ACCENT } else { ACCENT_DIM })
    };
    let hint = " Enter ";
    let lp = bw.saturating_sub(txt.chars().count() + hint.len()) / 2;
    let body = pad_right(&format!("{}{}", " ".repeat(lp), txt), bw.saturating_sub(hint.len()));
    let style = Style::new().fg(BG).bg(bg).add_modifier(if focused { Modifier::BOLD } else { Modifier::empty() });
    lines.push(Line::from(vec![
        Span::styled(body, style),
        Span::styled(hint, Style::new().fg(TEXT).bg(BG).add_modifier(Modifier::REVERSED)),
    ]));
    f.render_widget(Paragraph::new(lines), inner);
}

struct StepItem {
    name: String,
    time: String,
    state: ItemState,
}
#[derive(PartialEq)]
enum ItemState {
    Ok,
    Fail,
    Running,
}

fn execution(f: &mut Frame, app: &App, area: Rect) {
    let (t, c): (String, Color) = match &app.run {
        RunState::Idle => ("Execução".into(), TEXT),
        RunState::Running { .. } => (format!("{} Execução em andamento…", spinner(app.tick)), ACCENT),
        RunState::Failed(_) => ("✖ Execução falhou".into(), ERR),
        RunState::Done { report, .. } => match report.verdict {
            Verdict::NoIssuesDetected => ("✔ Execução concluída".into(), OK),
            Verdict::BaselineBroken => ("⚠ Execução concluída: baseline já falhava".into(), WARN),
            _ => ("✖ Execução concluída: problemas detectados".into(), ERR),
        },
    };
    let suffix = if app.show_details { "  ·  detalhes (d: voltar)" } else { "" };
    let block = panel_line(Line::from(vec![Span::styled(format!(" {t}{suffix} "), Style::new().fg(c).bold())]));
    let inner = block.inner(area).inner(&Margin { horizontal: 1, vertical: 0 });
    f.render_widget(block, area);

    if app.show_details {
        if let Some(r) = app.report() {
            let lines = report_view::lines(r);
            let max = lines.len().saturating_sub(1) as u16;
            let p = Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((app.scroll.min(max), 0));
            f.render_widget(p, inner);
            return;
        }
    }

    match &app.run {
        RunState::Idle => {
            let l = vec![
                Line::styled("Nenhuma execução ainda.", Style::new().fg(DIM)),
                Line::raw(""),
                Line::from(text("↑↓ escolhe o campo · ←→ muda a versão alvo · Ctrl+K troca de pacote.")),
                Line::from(text("Enter em “Iniciar Upgrade” cria o sandbox e testa o upgrade,")),
                Line::from(text("sem tocar nos ficheiros do projeto real.")),
            ];
            f.render_widget(Paragraph::new(l), inner);
        }
        RunState::Failed(e) => {
            let l: Vec<Line> = e.lines().map(|x| Line::styled(x.to_string(), Style::new().fg(ERR))).collect();
            f.render_widget(Paragraph::new(l).wrap(Wrap { trim: false }), inner);
        }
        RunState::Running { started, steps } => {
            let items = steps
                .iter()
                .map(|s| StepItem {
                    name: s.name.clone(),
                    time: secs(s.duration.unwrap_or_else(|| s.started.elapsed()).as_millis()),
                    state: if s.duration.is_some() { ItemState::Ok } else { ItemState::Running },
                })
                .collect();
            let done = steps.iter().filter(|s| s.duration.is_some()).count();
            let frac = (done as f64 + 0.5) / EXPECTED_STEPS as f64;
            steps_view(f, inner, items, frac.min(0.97), secs(started.elapsed().as_millis()), app.tick);
        }
        RunState::Done { report, .. } => {
            let n = report.steps.len();
            let items = report
                .steps
                .iter()
                .enumerate()
                .map(|(i, s)| StepItem {
                    name: s.name.clone(),
                    time: secs(s.duration_ms),
                    state: if i + 1 == n && report.verdict == Verdict::InstallFailed { ItemState::Fail } else { ItemState::Ok },
                })
                .collect();
            let total: u128 = report.steps.iter().map(|s| s.duration_ms).sum();
            steps_view(f, inner, items, 1.0, secs(total), app.tick);
        }
    }
}

fn steps_view(f: &mut Frame, inner: Rect, items: Vec<StepItem>, frac: f64, total: String, tick: usize) {
    let w = inner.width as usize;
    let reserve = 3usize; // barra + total (+ folga)
    let avail = (inner.height as usize).saturating_sub(reserve);
    let first = items.len().saturating_sub(avail);
    let mut lines: Vec<Line<'static>> = vec![];
    for (i, it) in items.iter().enumerate().skip(first) {
        let (icon, color) = match it.state {
            ItemState::Ok => ("✔".to_string(), OK),
            ItemState::Fail => ("✖".to_string(), ERR),
            ItemState::Running => (spinner(tick).to_string(), ACCENT),
        };
        let name = format!("{}. {}…", i + 1, it.name);
        let room = w.saturating_sub(it.time.chars().count() + 4);
        lines.push(split_line(
            vec![Span::styled(format!("{icon} "), Style::new().fg(color)), text(trunc(&name, room))],
            vec![Span::styled(it.time.clone(), Style::new().fg(if it.state == ItemState::Running { ACCENT } else { DIM }))],
            w,
        ));
    }
    let shown = lines.len() as u16;
    f.render_widget(Paragraph::new(lines), Rect { height: shown.max(1), ..inner });

    // Barra e total logo abaixo da lista de etapas (ou no fundo, se faltar espaço).
    let by = inner.y + (shown + 1).min(inner.height.saturating_sub(2));
    let filled = ((w as f64) * frac).round() as usize;
    let bar = Line::from(vec![
        Span::styled("━".repeat(filled.min(w)), Style::new().fg(ACCENT)),
        Span::styled("━".repeat(w.saturating_sub(filled)), Style::new().fg(BORDER)),
    ]);
    f.render_widget(Paragraph::new(bar), Rect { x: inner.x, y: by, width: inner.width, height: 1 });
    let pct = format!("{}%", (frac * 100.0).round() as u32);
    let line = split_line(vec![label(format!("Tempo total: {total}"))], vec![Span::styled(pct, Style::new().fg(ACCENT).bold())], w);
    f.render_widget(Paragraph::new(line), Rect { x: inner.x, y: by + 1, width: inner.width, height: 1 });
}

pub fn draw_picker(f: &mut Frame, app: &App, area: Rect) {
    let Some(p) = &app.picker else { return };
    let items = app.picker_items();
    let w = 62.min(area.width.saturating_sub(4));
    let h = (items.len() as u16 + 5).clamp(7, 16).min(area.height.saturating_sub(2));
    let r = Rect { x: area.x + (area.width - w) / 2, y: area.y + (area.height - h) / 2, width: w, height: h };
    f.render_widget(Clear, r);
    let block = panel("Selecionar dependência");
    let inner = block.inner(r).inner(&Margin { horizontal: 1, vertical: 0 });
    f.render_widget(Block::default().style(Style::new().bg(BG)), r);
    f.render_widget(block, r);

    let mut lines = vec![
        Line::from(vec![Span::styled("› ", Style::new().fg(ACCENT)), text(p.filter.clone()), Span::styled("▏", Style::new().fg(ACCENT))]),
        Line::styled("─".repeat(inner.width as usize), Style::new().fg(BORDER)),
    ];
    let rows = (inner.height as usize).saturating_sub(2);
    let start = p.cursor.saturating_sub(rows.saturating_sub(1));
    for (n, &i) in items.iter().enumerate().skip(start).take(rows) {
        let d = &app.deps[i];
        let ign = app.cfg.ignore.contains(&d.name);
        let mut spans = vec![
            Span::raw(if n == p.cursor { "▶ " } else { "  " }),
            Span::styled(pad_right(&d.name, 24), Style::new().fg(if ign { DIM } else { TEXT })),
            dim(pad_right(&d.current, 10)),
        ];
        if let Some(l) = &d.latest {
            spans.push(Span::raw("→ "));
            spans.push(Span::styled(l.clone(), Style::new().fg(WARN)));
        }
        if ign {
            spans.push(dim(" (ignorado)"));
        }
        if n == p.cursor {
            // Linha selecionada: um só estilo legível sobre o fundo de destaque.
            let sel = Style::new().fg(BG).bg(ACCENT).bold();
            let total: usize = spans.iter().map(|s| s.content.chars().count()).sum();
            let mut v: Vec<Span<'static>> = spans.into_iter().map(|s| Span::styled(s.content.to_string(), sel)).collect();
            v.push(Span::styled(" ".repeat((inner.width as usize).saturating_sub(total)), sel));
            lines.push(Line::from(v));
        } else {
            lines.push(Line::from(spans));
        }
    }
    if items.is_empty() {
        lines.push(Line::styled("nenhuma dependência corresponde", Style::new().fg(DIM)));
    }
    f.render_widget(Paragraph::new(lines), inner);
}
