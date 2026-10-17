//! Paleta e helpers de estilo (azul-marinho + verde-limão, como no design).
use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Borders};

pub const BG: Color = Color::Rgb(6, 18, 31);
pub const CHIP: Color = Color::Rgb(20, 54, 78);
pub const BORDER: Color = Color::Rgb(24, 78, 112);
pub const ACCENT: Color = Color::Rgb(184, 242, 58);
pub const ACCENT_DIM: Color = Color::Rgb(122, 160, 42);
pub const TEXT: Color = Color::Rgb(214, 226, 235);
pub const LABEL: Color = Color::Rgb(79, 185, 205);
pub const DIM: Color = Color::Rgb(104, 134, 150);
pub const OK: Color = Color::Rgb(120, 222, 96);
pub const ERR: Color = Color::Rgb(255, 88, 104);
pub const WARN: Color = Color::Rgb(255, 212, 64);

pub const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn spinner(tick: usize) -> &'static str {
    SPINNER[tick % SPINNER.len()]
}

pub fn panel(title: &str) -> Block<'static> {
    let b = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(BORDER));
    if title.is_empty() {
        b
    } else {
        b.title(Span::styled(format!(" {title} "), Style::new().fg(TEXT).bold()))
    }
}

pub fn panel_line(title: Line<'static>) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(BORDER))
        .title(title)
}

pub fn chip(s: &str) -> Span<'static> {
    Span::styled(s.to_string(), Style::new().fg(TEXT).bg(CHIP))
}
pub fn dim(s: impl Into<String>) -> Span<'static> {
    Span::styled(s.into(), Style::new().fg(DIM))
}
pub fn label(s: impl Into<String>) -> Span<'static> {
    Span::styled(s.into(), Style::new().fg(LABEL))
}
pub fn text(s: impl Into<String>) -> Span<'static> {
    Span::styled(s.into(), Style::new().fg(TEXT))
}
pub fn bold(s: impl Into<String>) -> Span<'static> {
    Span::styled(s.into(), Style::new().fg(TEXT).bold())
}

pub fn pad_right(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(w - n))
    }
}

/// Mantém o fim do texto (bom para caminhos): `…\Projetos\flowty`.
pub fn trunc_left(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max || max < 2 {
        return s.to_string();
    }
    let tail: String = s.chars().skip(n - (max - 1)).collect();
    format!("…{tail}")
}

pub fn trunc(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max || max < 2 {
        return s.to_string();
    }
    let head: String = s.chars().take(max - 1).collect();
    format!("{head}…")
}

pub fn secs(ms: u128) -> String {
    format!("{:.1}s", ms as f64 / 1000.0)
}

/// Uma linha com `left` à esquerda e `right` encostado à direita.
pub fn split_line(mut left: Vec<Span<'static>>, right: Vec<Span<'static>>, width: usize) -> Line<'static> {
    let lw: usize = left.iter().map(|s| s.content.chars().count()).sum();
    let rw: usize = right.iter().map(|s| s.content.chars().count()).sum();
    let gap = width.saturating_sub(lw + rw).max(1);
    left.push(Span::raw(" ".repeat(gap)));
    left.extend(right);
    Line::from(left)
}
