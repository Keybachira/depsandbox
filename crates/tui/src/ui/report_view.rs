//! Linhas de detalhe de um relatório (usadas no painel de execução e em "Ver Relatórios").
use crate::theme::*;
use depsandbox_core::{CheckStatus, FindingKind, Report, Severity, Verdict};
use ratatui::prelude::*;

pub fn status_span(s: CheckStatus) -> Span<'static> {
    match s {
        CheckStatus::Passed => Span::styled("✔ passou", Style::new().fg(OK)),
        CheckStatus::Failed => Span::styled("✖ falhou", Style::new().fg(ERR)),
        CheckStatus::Errored => Span::styled("✖ erro", Style::new().fg(ERR)),
        CheckStatus::Flaky => Span::styled("⚠ instável", Style::new().fg(WARN)),
        CheckStatus::Skipped => Span::styled("– ignorado", Style::new().fg(DIM)),
    }
}

pub fn lines(rep: &Report) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = vec![];
    let (txt, color) = match rep.verdict {
        Verdict::NoIssuesDetected => ("✔ nenhum problema detectado", OK),
        Verdict::BreakingChangesDetected => ("✖ breaking changes detectadas", ERR),
        Verdict::BaselineBroken => ("⚠ baseline já falhava (comparação pouco fiável)", WARN),
        Verdict::InstallFailed => ("✖ o upgrade não instalou", ERR),
    };
    out.push(Line::styled(format!("{}  {txt}", rep.target.spec()), Style::new().fg(color).bold()));
    out.push(Line::raw(""));
    out.push(Line::styled("Verificações (antes → depois)", Style::new().fg(TEXT).bold()));
    for b in &rep.baseline {
        let after = rep.after.iter().find(|a| a.name == b.name);
        out.push(Line::from(vec![
            Span::styled(format!("  {:<12}", b.name), Style::new().fg(TEXT)),
            status_span(b.status),
            dim("  →  "),
            after.map_or(dim("—"), |a| status_span(a.status)),
        ]));
    }
    if rep.baseline.is_empty() {
        for a in &rep.after {
            out.push(Line::from(vec![dim(format!("  {:<12}— → ", a.name)), status_span(a.status)]));
        }
    }
    out.push(Line::raw(""));
    out.push(Line::styled("Achados", Style::new().fg(TEXT).bold()));
    if rep.findings.is_empty() {
        out.push(Line::styled("  nenhum", Style::new().fg(DIM)));
    }
    let mut findings = rep.findings.clone();
    findings.sort_by_key(|f| std::cmp::Reverse(f.severity));
    for fi in &findings {
        let c = match fi.severity {
            Severity::Error => ERR,
            Severity::Warning => WARN,
            Severity::Info => OK,
        };
        let tag = match fi.kind {
            FindingKind::Regression => "regressão",
            FindingKind::Flaky => "instável",
            FindingKind::Fixed => "corrigido",
            FindingKind::PreExistingFailure => "já falhava",
            FindingKind::InstallFailure => "instalação",
        };
        out.push(Line::from(vec![Span::styled(format!("  [{tag}] "), Style::new().fg(c)), text(fi.message.clone())]));
    }
    for a in rep.after.iter().filter(|a| a.status != CheckStatus::Passed) {
        out.push(Line::raw(""));
        out.push(Line::styled(format!("Saída de `{}` ({})", a.name, a.command), Style::new().fg(TEXT).bold()));
        for l in a.output_tail.lines() {
            out.push(Line::styled(format!("  {l}"), Style::new().fg(DIM)));
        }
    }
    out
}
