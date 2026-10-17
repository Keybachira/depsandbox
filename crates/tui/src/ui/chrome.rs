use crate::app::{App, Screen};
use crate::theme::*;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

pub fn header(f: &mut Frame, area: Rect) {
    let block = panel("");
    let inner = block.inner(area);
    f.render_widget(block, area);
    let left = Line::from(vec![
        Span::styled(" ◆ ", Style::new().fg(ACCENT).bold()),
        Span::styled("depsandbox", Style::new().fg(TEXT).bold()),
        dim(format!("  v{}", env!("CARGO_PKG_VERSION"))),
        Span::raw("      "),
        label("Testa dependências. Evita problemas."),
    ]);
    f.render_widget(Paragraph::new(left), inner);
    let right = Line::from(vec![
        Span::styled("● ", Style::new().fg(ACCENT)),
        label("MODO INTERATIVO"),
        Span::styled("  │  ", Style::new().fg(BORDER)),
        Span::styled("? ", Style::new().fg(ACCENT)),
        text("Ajuda "),
    ]);
    f.render_widget(Paragraph::new(right).alignment(Alignment::Right), inner);
}

pub fn sidebar(f: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::vertical([Constraint::Min(8), Constraint::Length(if area.height >= 20 && area.width >= 27 { 7 } else { 0 })]).split(area);
    let w = rows[0].width.saturating_sub(2) as usize;
    let item = |s: Screen| -> Line<'static> {
        let n = s.index() + 1;
        if s == app.screen {
            let t = pad_right(&format!(" ▶ {}", s.name()), w);
            Line::styled(t, Style::new().fg(BG).bg(ACCENT).bold())
        } else {
            Line::from(vec![dim(format!(" {n} ")), text(s.name().to_string())])
        }
    };
    let mut lines = vec![Line::styled(" PRINCIPAL", Style::new().fg(DIM)), Line::raw("")];
    for s in &Screen::ALL[..4] {
        lines.push(item(*s));
        lines.push(Line::raw(""));
    }
    lines.push(Line::styled(" SUPORTE", Style::new().fg(DIM)));
    lines.push(Line::raw(""));
    for s in &Screen::ALL[4..] {
        lines.push(item(*s));
        lines.push(Line::raw(""));
    }
    f.render_widget(Paragraph::new(lines), rows[0].inner(&Margin { horizontal: 1, vertical: 1 }));

    if rows[1].height > 0 {
        let b = panel("");
        let inner = b.inner(rows[1]);
        f.render_widget(b, rows[1]);
        let l = vec![
            Line::from(vec![Span::styled("◆ ", Style::new().fg(ACCENT)), bold("depsandbox")]),
            Line::styled("Dependency Experiment Lab", Style::new().fg(LABEL)),
            Line::raw(""),
            Line::styled("Atualize com confiança.", Style::new().fg(DIM)),
            Line::styled("Não arrisque o projeto.", Style::new().fg(DIM)),
        ];
        f.render_widget(Paragraph::new(l), inner);
    }
}

fn key(k: &str) -> Span<'static> {
    chip(&format!(" {k} "))
}

pub fn footer(f: &mut Frame, app: &App, area: Rect) {
    let block = panel("");
    let inner = block.inner(area);
    f.render_widget(block, area);
    let w = inner.width as usize;

    let mut left: Vec<Span<'static>> = vec![
        Span::raw(" "),
        key("↑↓"),
        dim(" Navegar  "),
        key("Enter"),
        dim(" Selecionar  "),
        key("Esc"),
        dim(" Voltar  "),
        key("Tab"),
        dim(" Telas  "),
    ];
    if app.screen == Screen::Upgrade {
        left.push(key("Ctrl+K"));
        left.push(dim(" Pacote  "));
        if app.report().is_some() {
            left.push(key("d"));
            left.push(dim(" Detalhes  "));
        }
    }
    left.push(key("q"));
    left.push(dim(" Sair"));

    let cursor = if (app.tick / 5) % 2 == 0 { "_" } else { " " };
    let lw: usize = left.iter().map(|s| s.content.chars().count()).sum();
    let tail_w = 14; // "depsandbox _ "
    // O estado ocupa o espaço que sobrar depois das dicas; sem espaço, só as dicas.
    let avail = w.saturating_sub(lw + tail_w + 6);
    let mut right: Vec<Span<'static>> = vec![];
    if !app.status.is_empty() && avail >= 12 {
        right.push(Span::styled(trunc(&app.status, avail), Style::new().fg(LABEL)));
        right.push(dim("   │   "));
    }
    right.push(dim("depsandbox "));
    right.push(Span::styled(cursor, Style::new().fg(ACCENT)));
    right.push(Span::raw(" "));
    let line = split_line(left, right, w);
    f.render_widget(Paragraph::new(line), inner);
}
